module.exports = [
    {
        mode: 'development',
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
