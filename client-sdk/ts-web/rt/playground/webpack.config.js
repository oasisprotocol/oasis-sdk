const webpack = require('webpack');

module.exports = {
    mode: 'development',
    entry: {
        main: './src/index.js',
        consensus: './src/consensus.js',
    },
    resolve: {
        alias: {
            '@protobufjs/inquire': require.resolve('./src/errata/inquire'),
        },
        fallback: {
            stream: require.resolve('stream-browserify'),
        },
    },
    plugins: [
        new webpack.ProvidePlugin({
            process: 'process/browser',
            Buffer: ['buffer', 'Buffer'],
        }),
    ],
    output: {
        library: {
            name: 'playground',
            type: 'window',
            export: 'playground',
        },
    },
};
