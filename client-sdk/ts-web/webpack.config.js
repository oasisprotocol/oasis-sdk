const webpack = require('webpack');

module.exports = {
    mode: 'development',
    devServer: {
        contentBase: 'dist',
    },
    resolve: {fallback: {stream: require.resolve('stream-browserify')}},
    plugins: [
        new webpack.ProvidePlugin({
            process: 'process/browser.js',
            Buffer: ['buffer', 'Buffer'],
        }),
    ],
};
