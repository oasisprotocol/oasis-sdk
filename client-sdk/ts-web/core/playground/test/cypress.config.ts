import {defineConfig} from 'cypress';

export default defineConfig({
    video: false,
    e2e: {
        supportFile: false,
        setupNodeEvents(on, config) {
            on('task', {
                log(stringifiedArray) {
                    console.log('console.log', ...JSON.parse(stringifiedArray))
                    return null
                },
            })
        },
    },
});
