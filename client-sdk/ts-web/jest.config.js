module.exports = {
    testMatch: ['**/tests/**/*.ts'],
    testEnvironment: 'node',
    transform: {
        '^.+\\.[jt]s?$': 'babel-jest',
        // If you're using babel for both:
        // "^.+\\.[jt]sx?$": "babel-jest",
    },
};
