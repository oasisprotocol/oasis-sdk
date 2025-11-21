module.exports = {
    testMatch: ['**/test/**/*.ts'],
    testEnvironment: 'node',
    transform: {
        '^.+\\.ts$': ['ts-jest', {tsconfig: 'tsconfig.test.json'}],
    },
    modulePathIgnorePatterns: ['<rootDir>/dist'],
    moduleNameMapper: {
        '^(\\.{1,2}/.*)\\.js$': '$1',
    },
};
