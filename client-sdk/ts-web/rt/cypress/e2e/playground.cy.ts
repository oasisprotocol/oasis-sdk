describe('playground.cy.ts', () => {
    it('should finish simple-keyvalue', () => {
        cy.visit('http://localhost:8080/');
        // This is similar to `.its('playground')`, except that
        // (i) it doesn't retry if `w.playground` rejects, and
        // (ii) it passes when `w.playground` fulfills with `undefined`.
        cy.window()
            .then({timeout: 120_000}, (w) => w.playground)
            .then((w) => {
                expect(w.playground).to.be.ok;
            });
    });

    it('should finish simple-consensus', () => {
        cy.visit('http://localhost:8080/consensus.html');
        // This is similar to `.its('playground')`, except that
        // (i) it doesn't retry if `w.playground` rejects, and
        // (ii) it passes when `w.playground` fulfills with `undefined`.
        cy.window()
            .then({timeout: 120_000}, (w) => w.playground)
            .then((w) => {
                expect(w.playground).to.be.ok;
            });
    });
});
