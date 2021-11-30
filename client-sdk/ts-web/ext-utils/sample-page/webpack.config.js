module.exports = {
    mode: 'development',
    resolve: {
        alias: {
            '@protobufjs/inquire': require.resolve('./src/errata/inquire'),
        },
        fallback: {
            stream: require.resolve('stream-browserify'),
        },
    },
    output: {
        library: {
            name: 'playground',
            type: 'window',
            export: 'playground',
        },
    },
};
