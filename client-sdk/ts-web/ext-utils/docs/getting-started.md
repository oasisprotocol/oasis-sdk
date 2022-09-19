# Getting started

## Building a dApp

**The sample dApp.**
Take a look at the sample code for the dApp side of the overall interface.
That's in [sample-page/src/index.js](../sample-page/src/index.js).

At a high level, the sample page opens an `oasisExt.connection.ExtConnection`,
specifying what extension it wants to interact with, in the form of an origin
string.
This adds an `iframe` to the page, so you must call it after the `body`
element exists, i.e. not in a script in the `head` without `defer`.
It uses functions and classes in `oasisExt`, which require a `connection`
parameter.

**Your own dApp.**
Use the `@oasisprotocol/client` and/or `@oasisprotocol/client-rt` to write
the code for preparing the body of a transaction and submitting it.
Use this package to connect to an extension.
Find a key to use with `oasisExt.keys.list`, identified by a "which" value
(the format may vary depending on the extension).
Then, get the public key and sign the transaction using
`oasisExt.signature.ExtContextSigner`.

## Building an extension

**The sample extension.**
Take a look at the sample code for the extension side of the overall dApp
interface and signer interface.
That's in [sample-ext/src/index.js](../sample-ext/src/index.js).
It's integrated into the extension here in
[manifest.json](../sample-ext/manifest.json#L8), as a "web accessible
resource."

At a high level, the sample extension calls an `oasisExt.ext.ready` function
with a bunch of
callbacks, which the `oasisExt` library will call when the dApp makes
requests.
The sample extension handles these entirely within a little `iframe` that a
dApp will embed, but it's fine to relay messages to a background page or
otherwise communicate with other parts of the extension.

There's a webpack build step for the sample extension.
Run `npm run sample-ext` in `client-sdk/ts-web/ext-utils` to do that step.
Then, load the `client-sdk/ts-web/ext-utils` directory as an unpacked
extension.

**Trying your own extension.**
Integrate a similar "web accessible resource" page into your extension and
implement the extension side of the signer interface.
To try it out with the sample dApp page, do the following:

1. Get your extension ID from [chrome://extensions](chrome://extensions), e.g.
   `joglombbipnjdfbkimehokiomlbhcobn` here:

   ![](sample-ext-id.png)

2. Build the [sample-page](../sample-page) webpack project.
   You should be able to use `npm run sample-page` in
   `client-sdk/ts-web/ext-utils` in a checkout of this repo to build it and
   serve it locally.

   ![](sample-page-webpack.png)

3. Open the URL
   `http://localhost:8080/?ext=chrome-extension://(your extension id)` in the
   browser.

   ![](sample-page-open.png)

   This should ask the extension (1) to list the keys, (2) to sign a consensus
   staking transfer, and (3) to sign a runtime accounts transfer.
