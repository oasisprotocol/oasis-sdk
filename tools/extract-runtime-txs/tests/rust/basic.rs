//! Smart contracts module.

#[sdk_derive(MethodHandler)]
impl<Cfg: Config> Module<Cfg> {
    #[handler(call = "contracts.Upload")]
    pub fn tx_upload<C: TxContext>(
        ctx: &mut C,
        body: types::Upload,
    ) -> Result<types::UploadResult, Error> {
        let params = Self::params(ctx.runtime_state());
        let uploader = ctx.tx_caller_address();

        // Validate code size.
        let code_size: u32 = body
            .code
            .len()
            .try_into()
            .map_err(|_| Error::CodeTooLarge(u32::MAX, params.max_code_size))?;
        if code_size > params.max_code_size {
            return Err(Error::CodeTooLarge(code_size, params.max_code_size));
        }

        // Account for base gas.
        <C::Runtime as Runtime>::Core::use_tx_gas(ctx, params.gas_costs.tx_upload)?;
        <C::Runtime as Runtime>::Core::use_tx_gas(
            ctx,
            params
                .gas_costs
                .tx_upload_per_byte
                .saturating_mul(body.code.len() as u64),
        )?;

        // Decompress code.
        let mut code = Vec::with_capacity(body.code.len());
        let decoder = snap::read::FrameDecoder::new(body.code.as_slice());
        decoder
            .take(params.max_code_size.into())
            .read_to_end(&mut code)
            .map_err(|_| Error::CodeMalformed)?;

        // Account for extra gas needed after decompression.
        let plain_code_size: u32 = code.len().try_into().unwrap();
        <C::Runtime as Runtime>::Core::use_tx_gas(
            ctx,
            params
                .gas_costs
                .tx_upload_per_byte
                .saturating_mul(plain_code_size.saturating_sub(code_size) as u64),
        )?;

        if ctx.is_check_only() || (ctx.is_simulation() && !ctx.are_expensive_queries_allowed()) {
            // Only fast checks are allowed.
            return Ok(types::UploadResult::default());
        }

        // Validate and transform the code.
        let code = wasm::validate_and_transform::<Cfg, C>(&code, body.abi)?;
        let hash = Hash::digest_bytes(&code);

        // Validate code size again and account for any instrumentation. This is here to avoid any
        // incentives in generating code that gets maximally inflated after instrumentation.
        let inst_code_size: u32 = code
            .len()
            .try_into()
            .map_err(|_| Error::CodeTooLarge(u32::MAX, params.max_code_size))?;
        if inst_code_size > params.max_code_size {
            return Err(Error::CodeTooLarge(inst_code_size, params.max_code_size));
        }
        <C::Runtime as Runtime>::Core::use_tx_gas(
            ctx,
            params
                .gas_costs
                .tx_upload_per_byte
                .saturating_mul(inst_code_size.saturating_sub(plain_code_size) as u64),
        )?;

        // Assign next identifier.
        let mut store = storage::PrefixStore::new(ctx.runtime_state(), &MODULE_NAME);
        let mut tstore = storage::TypedStore::new(&mut store);
        let id: types::CodeId = tstore.get(state::NEXT_CODE_IDENTIFIER).unwrap_or_default();
        tstore.insert(state::NEXT_CODE_IDENTIFIER, id.increment());

        // Store information about uploaded code.
        let code_info = types::Code {
            id,
            hash,
            abi: body.abi,
            uploader,
            instantiate_policy: body.instantiate_policy,
        };
        Self::store_code(ctx, &code_info, &code)?;
        Self::store_code_info(ctx, code_info)?;

        Ok(types::UploadResult { id })
    }

    #[handler(query = "contracts.Code")]
    pub fn query_code<C: Context>(
        ctx: &mut C,
        args: types::CodeQuery,
    ) -> Result<types::Code, Error> {
        Self::load_code_info(ctx, args.id)
    }
}
