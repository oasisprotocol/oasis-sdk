/// <reference types="cypress" />

describe('playground', () => {
    it('finishes', () => {
        cy.visit('http://localhost:8080/');
        cy.contains('barolitopsis', {timeout: 60_000});
    });
});
