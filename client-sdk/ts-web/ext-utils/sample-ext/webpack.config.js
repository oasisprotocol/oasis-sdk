const webpack = require('webpack');

module.exports = {
    mode: 'development',
    resolve: { fallback: { stream: require.resolve('stream-browserify') } },
    devtool: false,
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
    // In tests, we serve the extension files as a plain old website.
    devServer: {
        devMiddleware: {
            publicPath: '/dist',
        },
        port: 8081,
        static: '.',
    },
};
