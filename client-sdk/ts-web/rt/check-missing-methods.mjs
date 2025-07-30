import { execSync } from 'node:child_process'

function events() {
  // grep -P "const EVENT_.* = \K\d+" -r ./client-sdk/ts-web/rt --exclude-dir=node_modules --exclude-dir=playground --exclude-dir=dist
  // grep -oP 'sdk_event\(code = \K\d+' -r ./runtime-sdk/ | grep -oP "modules/\K.*"
  // grep -n1 -P 'sdk_event\(code = \K\d+' -r ./runtime-sdk/ | grep -oP "modules/\K.*"
  // grep -ozP '(?s)sdk_event\(code = \d+.+?{.*?\n' -r ./runtime-sdk/ | grep -zoP "(?s)modules/\K.*"

  // #[sdk_event(code = 3)]
  // AppRemoved { id: AppId },
  const grepCommand = String.raw`grep -ozP '(?s)sdk_event\(code = \d+.+?{.*?\n' -r ./runtime-sdk/ | grep -zoP "(?s)modules/\K.*"`
  const grepResult = execSync(grepCommand, {cwd: '../../..'}).toString().slice(0, -1)
  // console.log(grepResult)
  const generated = grepResult.split('\0').map(e=>{
    const moduleName = e.split('/')[0].replace('-', '')
    const eventName = e.replace(/\n/g, '').match(/](.*)\{/)?.[1].trim()
    const eventCode = e.match(/\d+/)?.[0]
    const upperCaseName = 'EVENT' + eventName?.replace(/[A-Z]/g, letter => `_${letter.toLowerCase()}`).toUpperCase() + '_CODE'
    return `${moduleName}.ts:export const ${upperCaseName} = ${eventCode};`
  })

  const grepTsCommand = String.raw`grep -P "const EVENT_.* = \K\d+" -r ./client-sdk/ts-web/rt/src | grep -oP 'ts-web/rt/src/\K.*'`
  const existing = execSync(grepTsCommand, {cwd: '../../..'}).toString().trim().split('\n')
  console.log('missing events:\n', generated.filter(e => !existing.includes(e)).join('\n'))
  console.log('extraneous events:\n', existing.filter(e => !generated.includes(e)).join('\n'))
}
events()

function errors() {
  // #[sdk_error(code = 10)]
  // InvalidSignedSimulateCall(&'static str),
  const grepCommand = String.raw`grep -ozP '(?s)sdk_error\(code = \d+.+?\]\s+\w+' -r ./runtime-sdk/ | grep -zoP "(?s)modules/\K.*"`
  const grepResult = execSync(grepCommand, {cwd: '../../..'}).toString().slice(0, -1)
  // console.log(grepResult.split('\0'))
  const generated = grepResult.split('\0').map(e=>{
    const moduleName = e.split('/')[0].replace('-', '')
    const eventName = e.match(/\w+$/)?.[0]
    const eventCode = e.match(/code = (\d+)/)?.[1]
    let upperCaseName = 'ERR' + eventName?.replace(/[A-Z]/g, letter => `_${letter.toLowerCase()}`).toUpperCase() + '_CODE'
    upperCaseName = upperCaseName.replace('A_B_I', 'ABI').replace('E_R_C20', 'ERC20').replace('R_A_K', 'RAK')
    return `${moduleName}.ts:export const ${upperCaseName} = ${eventCode};`
  })

  const grepTsCommand = String.raw`grep -P "const ERR_.* = \K\d+" -r ./client-sdk/ts-web/rt/src | grep -oP 'ts-web/rt/src/\K.*'`
  const existing = execSync(grepTsCommand, {cwd: '../../..'}).toString().trim().split('\n')
  console.log('missing errors:\n', generated.filter(e => !existing.includes(e)).join('\n'))
  console.log('extraneous errors:\n', existing.filter(e => !generated.includes(e)).join('\n'))
}
errors()

function methods() {
  const grepCommand = String.raw`
    comm -1 -3 \
      <(grep --no-filename -oP "const METHOD_.* = '\K[^']+" -r ./client-sdk/ts-web/rt/src | sort) \
      <(grep --no-filename -oP 'handler\((call|query) = "\K.*' -r ./runtime-sdk/ | grep -vP "internal|test\.|alphabet\." | grep -oP '^[^"]+' | sort)
  `
  const grepResult = execSync(grepCommand, {cwd: '../../..', shell: '/bin/bash'}).toString()
  console.log('missing methods:\n', grepResult)
  console.log('not checked: <module>.Parameters')
}
methods()
