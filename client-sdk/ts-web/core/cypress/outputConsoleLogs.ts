/** Usage: add to cypress config */
export const setupNodeEvents: Cypress.Config['setupNodeEvents'] = (on, config) => {
    on('task', {
        consoleLog(stringifiedArray) {
            console.log('console.log', ...JSON.parse(stringifiedArray))
            return null
        },
        consoleWarn(stringifiedArray) {
            console.warn('console.warn', ...JSON.parse(stringifiedArray))
            return null
        },
        consoleError(stringifiedArray) {
            console.error('console.error', ...JSON.parse(stringifiedArray))
            return null
        },
    });
};

/** Usage: call before a test */
export function beforeWindowLoadListener() {
    before(() => {
        Cypress.on('window:before:load', (w) => {
            cy.stub(w.console, 'log').callsFake((...args) => consoleTask('consoleLog', ...args));
            cy.stub(w.console, 'warn').callsFake((...args) => consoleTask('consoleWarn', ...args));
            cy.stub(w.console, 'error').callsFake((...args) => consoleTask('consoleError', ...args));
        });
    });
}

/**
 * Workaround: `cy.task('consoleLog', stringifiedArray)` throws because cypress command is inside cypress command.
 */
function consoleTask(taskName: 'consoleLog' | 'consoleWarn' | 'consoleError', ...args: any[]) {
    const stringifiedArray = JSON.stringify(args, (key, value) => (typeof value === 'bigint' ? `${value}n` : value), 2);

    Cypress.emit(
        'backend:request',
        'task',
        { task: taskName, arg: stringifiedArray },
        () => {},
    );
}
