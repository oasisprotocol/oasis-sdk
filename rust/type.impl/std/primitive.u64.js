(function() {var type_impls = {
"oasis_core_runtime":[["<details class=\"toggle implementors-toggle\" open><summary><section id=\"impl-From%3CVersion%3E-for-u64\" class=\"impl\"><a class=\"src rightside\" href=\"src/oasis_core_runtime/common/version.rs.html#45-49\">source</a><a href=\"#impl-From%3CVersion%3E-for-u64\" class=\"anchor\">§</a><h3 class=\"code-header\">impl <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/convert/trait.From.html\" title=\"trait core::convert::From\">From</a>&lt;<a class=\"struct\" href=\"oasis_core_runtime/common/version/struct.Version.html\" title=\"struct oasis_core_runtime::common::version::Version\">Version</a>&gt; for <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/std/primitive.u64.html\">u64</a></h3></section></summary><div class=\"impl-items\"><details class=\"toggle method-toggle\" open><summary><section id=\"method.from\" class=\"method trait-impl\"><a class=\"src rightside\" href=\"src/oasis_core_runtime/common/version.rs.html#46-48\">source</a><a href=\"#method.from\" class=\"anchor\">§</a><h4 class=\"code-header\">fn <a href=\"https://doc.rust-lang.org/nightly/core/convert/trait.From.html#tymethod.from\" class=\"fn\">from</a>(val: <a class=\"struct\" href=\"oasis_core_runtime/common/version/struct.Version.html\" title=\"struct oasis_core_runtime::common::version::Version\">Version</a>) -&gt; Self</h4></section></summary><div class='docblock'>Converts to this type from the input type.</div></details></div></details>","From<Version>","oasis_core_runtime::consensus::beacon::EpochTime","oasis_core_runtime::consensus::transaction::Gas"],["<details class=\"toggle implementors-toggle\" open><summary><section id=\"impl-KeyFormatAtom-for-u64\" class=\"impl\"><a class=\"src rightside\" href=\"src/oasis_core_runtime/common/key_format.rs.html#95-110\">source</a><a href=\"#impl-KeyFormatAtom-for-u64\" class=\"anchor\">§</a><h3 class=\"code-header\">impl <a class=\"trait\" href=\"oasis_core_runtime/common/key_format/trait.KeyFormatAtom.html\" title=\"trait oasis_core_runtime::common::key_format::KeyFormatAtom\">KeyFormatAtom</a> for <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/std/primitive.u64.html\">u64</a></h3></section></summary><div class=\"impl-items\"><section id=\"method.size\" class=\"method trait-impl\"><a class=\"src rightside\" href=\"src/oasis_core_runtime/common/key_format.rs.html#96-98\">source</a><a href=\"#method.size\" class=\"anchor\">§</a><h4 class=\"code-header\">fn <a href=\"oasis_core_runtime/common/key_format/trait.KeyFormatAtom.html#tymethod.size\" class=\"fn\">size</a>() -&gt; <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/std/primitive.usize.html\">usize</a></h4></section><section id=\"method.encode_atom\" class=\"method trait-impl\"><a class=\"src rightside\" href=\"src/oasis_core_runtime/common/key_format.rs.html#100-102\">source</a><a href=\"#method.encode_atom\" class=\"anchor\">§</a><h4 class=\"code-header\">fn <a href=\"oasis_core_runtime/common/key_format/trait.KeyFormatAtom.html#tymethod.encode_atom\" class=\"fn\">encode_atom</a>(self) -&gt; <a class=\"struct\" href=\"https://doc.rust-lang.org/nightly/alloc/vec/struct.Vec.html\" title=\"struct alloc::vec::Vec\">Vec</a>&lt;<a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/std/primitive.u8.html\">u8</a>&gt; <a href=\"#\" class=\"tooltip\" data-notable-ty=\"Vec&lt;u8&gt;\">ⓘ</a></h4></section><section id=\"method.decode_atom\" class=\"method trait-impl\"><a class=\"src rightside\" href=\"src/oasis_core_runtime/common/key_format.rs.html#104-109\">source</a><a href=\"#method.decode_atom\" class=\"anchor\">§</a><h4 class=\"code-header\">fn <a href=\"oasis_core_runtime/common/key_format/trait.KeyFormatAtom.html#tymethod.decode_atom\" class=\"fn\">decode_atom</a>(data: &amp;[<a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/std/primitive.u8.html\">u8</a>]) -&gt; Self<div class=\"where\">where\n    Self: <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Sized.html\" title=\"trait core::marker::Sized\">Sized</a>,</div></h4></section></div></details>","KeyFormatAtom","oasis_core_runtime::consensus::beacon::EpochTime","oasis_core_runtime::consensus::transaction::Gas"],["<details class=\"toggle implementors-toggle\" open><summary><section id=\"impl-Marshal-for-u64\" class=\"impl\"><a class=\"src rightside\" href=\"src/oasis_core_runtime/storage/mkvs/marshal.rs.html#50-66\">source</a><a href=\"#impl-Marshal-for-u64\" class=\"anchor\">§</a><h3 class=\"code-header\">impl <a class=\"trait\" href=\"oasis_core_runtime/storage/mkvs/marshal/trait.Marshal.html\" title=\"trait oasis_core_runtime::storage::mkvs::marshal::Marshal\">Marshal</a> for <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/std/primitive.u64.html\">u64</a></h3></section></summary><div class=\"impl-items\"><details class=\"toggle method-toggle\" open><summary><section id=\"method.marshal_binary\" class=\"method trait-impl\"><a class=\"src rightside\" href=\"src/oasis_core_runtime/storage/mkvs/marshal.rs.html#51-55\">source</a><a href=\"#method.marshal_binary\" class=\"anchor\">§</a><h4 class=\"code-header\">fn <a href=\"oasis_core_runtime/storage/mkvs/marshal/trait.Marshal.html#tymethod.marshal_binary\" class=\"fn\">marshal_binary</a>(&amp;self) -&gt; <a class=\"type\" href=\"https://docs.rs/anyhow/1.0.81/anyhow/type.Result.html\" title=\"type anyhow::Result\">Result</a>&lt;<a class=\"struct\" href=\"https://doc.rust-lang.org/nightly/alloc/vec/struct.Vec.html\" title=\"struct alloc::vec::Vec\">Vec</a>&lt;<a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/std/primitive.u8.html\">u8</a>&gt;&gt;</h4></section></summary><div class='docblock'>Marshal the object into a binary form and return it as a new vector.</div></details><details class=\"toggle method-toggle\" open><summary><section id=\"method.unmarshal_binary\" class=\"method trait-impl\"><a class=\"src rightside\" href=\"src/oasis_core_runtime/storage/mkvs/marshal.rs.html#57-65\">source</a><a href=\"#method.unmarshal_binary\" class=\"anchor\">§</a><h4 class=\"code-header\">fn <a href=\"oasis_core_runtime/storage/mkvs/marshal/trait.Marshal.html#tymethod.unmarshal_binary\" class=\"fn\">unmarshal_binary</a>(&amp;mut self, data: &amp;[<a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/std/primitive.u8.html\">u8</a>]) -&gt; <a class=\"type\" href=\"https://docs.rs/anyhow/1.0.81/anyhow/type.Result.html\" title=\"type anyhow::Result\">Result</a>&lt;<a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/std/primitive.usize.html\">usize</a>&gt;</h4></section></summary><div class='docblock'>Unmarshal from the given byte slice reference and modify <code>self</code>.</div></details></div></details>","Marshal","oasis_core_runtime::consensus::beacon::EpochTime","oasis_core_runtime::consensus::transaction::Gas"],["<details class=\"toggle implementors-toggle\" open><summary><section id=\"impl-TryFrom%3C%26Quantity%3E-for-u64\" class=\"impl\"><a class=\"src rightside\" href=\"src/oasis_core_runtime/common/quantity.rs.html#83-89\">source</a><a href=\"#impl-TryFrom%3C%26Quantity%3E-for-u64\" class=\"anchor\">§</a><h3 class=\"code-header\">impl <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/convert/trait.TryFrom.html\" title=\"trait core::convert::TryFrom\">TryFrom</a>&lt;&amp;<a class=\"struct\" href=\"oasis_core_runtime/common/quantity/struct.Quantity.html\" title=\"struct oasis_core_runtime::common::quantity::Quantity\">Quantity</a>&gt; for <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/std/primitive.u64.html\">u64</a></h3></section></summary><div class=\"impl-items\"><details class=\"toggle\" open><summary><section id=\"associatedtype.Error\" class=\"associatedtype trait-impl\"><a href=\"#associatedtype.Error\" class=\"anchor\">§</a><h4 class=\"code-header\">type <a href=\"https://doc.rust-lang.org/nightly/core/convert/trait.TryFrom.html#associatedtype.Error\" class=\"associatedtype\">Error</a> = <a class=\"enum\" href=\"https://doc.rust-lang.org/nightly/core/num/error/enum.IntErrorKind.html\" title=\"enum core::num::error::IntErrorKind\">IntErrorKind</a></h4></section></summary><div class='docblock'>The type returned in the event of a conversion error.</div></details><details class=\"toggle method-toggle\" open><summary><section id=\"method.try_from\" class=\"method trait-impl\"><a class=\"src rightside\" href=\"src/oasis_core_runtime/common/quantity.rs.html#86-88\">source</a><a href=\"#method.try_from\" class=\"anchor\">§</a><h4 class=\"code-header\">fn <a href=\"https://doc.rust-lang.org/nightly/core/convert/trait.TryFrom.html#tymethod.try_from\" class=\"fn\">try_from</a>(value: &amp;<a class=\"struct\" href=\"oasis_core_runtime/common/quantity/struct.Quantity.html\" title=\"struct oasis_core_runtime::common::quantity::Quantity\">Quantity</a>) -&gt; <a class=\"enum\" href=\"https://doc.rust-lang.org/nightly/core/result/enum.Result.html\" title=\"enum core::result::Result\">Result</a>&lt;<a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/std/primitive.u64.html\">u64</a>, Self::<a class=\"associatedtype\" href=\"https://doc.rust-lang.org/nightly/core/convert/trait.TryFrom.html#associatedtype.Error\" title=\"type core::convert::TryFrom::Error\">Error</a>&gt;</h4></section></summary><div class='docblock'>Performs the conversion.</div></details></div></details>","TryFrom<&Quantity>","oasis_core_runtime::consensus::beacon::EpochTime","oasis_core_runtime::consensus::transaction::Gas"],["<details class=\"toggle implementors-toggle\" open><summary><section id=\"impl-TryFrom%3CQuantity%3E-for-u64\" class=\"impl\"><a class=\"src rightside\" href=\"src/oasis_core_runtime/common/quantity.rs.html#75-81\">source</a><a href=\"#impl-TryFrom%3CQuantity%3E-for-u64\" class=\"anchor\">§</a><h3 class=\"code-header\">impl <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/convert/trait.TryFrom.html\" title=\"trait core::convert::TryFrom\">TryFrom</a>&lt;<a class=\"struct\" href=\"oasis_core_runtime/common/quantity/struct.Quantity.html\" title=\"struct oasis_core_runtime::common::quantity::Quantity\">Quantity</a>&gt; for <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/std/primitive.u64.html\">u64</a></h3></section></summary><div class=\"impl-items\"><details class=\"toggle\" open><summary><section id=\"associatedtype.Error\" class=\"associatedtype trait-impl\"><a href=\"#associatedtype.Error\" class=\"anchor\">§</a><h4 class=\"code-header\">type <a href=\"https://doc.rust-lang.org/nightly/core/convert/trait.TryFrom.html#associatedtype.Error\" class=\"associatedtype\">Error</a> = <a class=\"enum\" href=\"https://doc.rust-lang.org/nightly/core/num/error/enum.IntErrorKind.html\" title=\"enum core::num::error::IntErrorKind\">IntErrorKind</a></h4></section></summary><div class='docblock'>The type returned in the event of a conversion error.</div></details><details class=\"toggle method-toggle\" open><summary><section id=\"method.try_from\" class=\"method trait-impl\"><a class=\"src rightside\" href=\"src/oasis_core_runtime/common/quantity.rs.html#78-80\">source</a><a href=\"#method.try_from\" class=\"anchor\">§</a><h4 class=\"code-header\">fn <a href=\"https://doc.rust-lang.org/nightly/core/convert/trait.TryFrom.html#tymethod.try_from\" class=\"fn\">try_from</a>(value: <a class=\"struct\" href=\"oasis_core_runtime/common/quantity/struct.Quantity.html\" title=\"struct oasis_core_runtime::common::quantity::Quantity\">Quantity</a>) -&gt; <a class=\"enum\" href=\"https://doc.rust-lang.org/nightly/core/result/enum.Result.html\" title=\"enum core::result::Result\">Result</a>&lt;<a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/std/primitive.u64.html\">u64</a>, Self::<a class=\"associatedtype\" href=\"https://doc.rust-lang.org/nightly/core/convert/trait.TryFrom.html#associatedtype.Error\" title=\"type core::convert::TryFrom::Error\">Error</a>&gt;</h4></section></summary><div class='docblock'>Performs the conversion.</div></details></div></details>","TryFrom<Quantity>","oasis_core_runtime::consensus::beacon::EpochTime","oasis_core_runtime::consensus::transaction::Gas"]]
};if (window.register_type_impls) {window.register_type_impls(type_impls);} else {window.pending_type_impls = type_impls;}})()