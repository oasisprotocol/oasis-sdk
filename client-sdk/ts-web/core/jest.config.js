module.exports = {
  // testEnvironment: 'node',
  moduleNameMapper: {
    // jest-resolve doesn't support `exports` field in package.json. unhack
    // this when it does.
    'cborg': '<rootDir>/../node_modules/cborg/cjs/cborg.js',
  },
};
