module.exports = {
    // We load this file from the packages' jest.config.js files (e.g. see
    // core/jest.config.js), so it'll be resolved relative to the package's
    // directory.
    testMatch: ['**/test/**/*.ts'],
    testEnvironment: 'node',
    preset: 'ts-jest'
};
