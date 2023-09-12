import {defineConfig} from 'cypress';
import * as outputConsoleLogs from './cypress/outputConsoleLogs';

export default defineConfig({
    e2e: {
        supportFile: false,
        setupNodeEvents: outputConsoleLogs.setupNodeEvents,
    },
});
