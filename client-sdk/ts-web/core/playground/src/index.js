// @ts-check

import {startPlayground} from './startPlayground.mjs';

export const playground = startPlayground();

playground.catch((e) => {
    console.error(e);
});
