import { fromBigInt } from '../src/quantity';
import { addressFromBech32, transferWrapper } from '../src/staking';

describe('types', () => {
    describe('transferWrapper', () => {
        const amount = fromBigInt(1000n)
        const to = addressFromBech32('oasis1qqx0wgxjwlw3jwatuwqj6582hdm9rjs4pcnvzz66')

        it('Should expect fields "amount" and "to"', () => {
            const tw = transferWrapper()
            tw.setBody({ amount, to })
            expect(tw.transaction.body?.amount).toEqual(amount)
            expect(tw.transaction.body?.to).toEqual(to)
        })

        it('Should detect incorrect fields', async () => {
            const tw = transferWrapper()
            tw.setBody({
                amount,
                to,
                // @ts-expect-error Expect typescript to detect incorrect field
                otherField: 5,
            })
            // @ts-expect-error Expect typescript to detect incorrect field
            expect(tw.transaction.body?.otherField).toEqual(5)
        });
    });
});
