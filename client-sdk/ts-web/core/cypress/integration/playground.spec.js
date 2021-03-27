/// <reference types="cypress" />

describe('playground', () => {
    it('finishes', () => {
        cy.visit('http://localhost:42280/');
        // This is similar to `.its('playground')`, except that
        // (i) it doesn't retry if `w.playground` rejects, and
        // (ii) it passes when `w.playground` fulfills with `undefined`.
        cy.window().then({timeout: 120_000}, (w) => w.playground);
    });
});
