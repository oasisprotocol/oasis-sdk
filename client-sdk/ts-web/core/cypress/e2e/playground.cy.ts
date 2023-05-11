function log(...args) {
    const stringifiedArray = JSON.stringify(args, (key, value) => (typeof value === 'bigint' ? value.toString() : value), 2);

    // Workaround: `cy.task('log', stringifiedArray)` throws that cypress command is inside cypress command.
    Cypress.emit(
        'backend:request',
        'task',
        { task: 'log', arg: stringifiedArray },
        () => {},
    );
}

describe('playground.cy.ts', () => {
    it('should finish', () => {
        cy.visit('http://localhost:8080/', {
            onBeforeLoad(w) {
                cy.stub(w.console, 'log').callsFake((...args) => log(...args));
            },
        });
        // This is similar to `.its('playground')`, except that
        // (i) it doesn't retry if `w.playground` rejects, and
        // (ii) it passes when `w.playground` fulfills with `undefined`.
        cy.window().then({timeout: 120_000}, (w) => w.playground)
    });
});
