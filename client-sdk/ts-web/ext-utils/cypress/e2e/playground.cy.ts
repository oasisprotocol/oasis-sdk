describe('playground.cy.ts', () => {
    it('should finish', () => {
        cy.visit('http://localhost:8080/?ext=http://localhost:8081&test_noninteractive=1');
        // This is similar to `.its('playground')`, except that
        // (i) it doesn't retry if `w.playground` rejects, and
        // (ii) it passes when `w.playground` fulfills with `undefined`.
        cy.window().then({timeout: 4_000}, (w) => w.playground);
    });
});
