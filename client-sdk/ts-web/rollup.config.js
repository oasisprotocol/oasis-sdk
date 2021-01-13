import resolve from '@rollup/plugin-node-resolve'
import commonjs from '@rollup/plugin-commonjs'

export default {
    input: 'src/index.js',
    plugins: [
        resolve(),
        commonjs(),
    ],
    output: {
        file: 'dist/main.js'
    },
}
