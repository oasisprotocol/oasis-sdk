module.exports = {
    mode: 'development',
    resolve: { fallback: { stream: require.resolve('stream-browserify') } },
    output: {
        library: {
            name: 'playground',
            type: 'window',
            export: 'playground',
        },
    },
};
