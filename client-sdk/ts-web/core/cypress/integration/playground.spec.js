/// <reference types="cypress" />

describe('playground', () => {
    it('finishes', () => {
        cy.visit('http://localhost:42280/');
        cy.contains('plastrophonic', {timeout: 60_000});
    });
});
