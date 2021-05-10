const webpack = require('webpack');

module.exports = [
    {
        mode: 'development',
        resolve: {fallback: {stream: require.resolve('stream-browserify')}},
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
    },
    {
        mode: 'development',
        resolve: {fallback: {stream: require.resolve('stream-browserify')}},
        plugins: [
            new webpack.ProvidePlugin({
                process: 'process/browser',
                Buffer: ['buffer', 'Buffer'],
            }),
        ],
        entry: './src/consensus.js',
        output: {
            library: {
                name: 'playground',
                type: 'window',
                export: 'playground',
            },
            filename: 'consensus.js',
        },
    },
];
