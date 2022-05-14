import collections
import json
import re
import sys

with open('../../../../target/doc/oasis_runtime_sdk.json', 'r') as crate_file:
    crate = json.load(crate_file)

UsedType = collections.namedtuple('UsedType', ('ref', 'source'))

used_types_by_rdid = {}

this_crate_id = 0
external_crate_ids_by_name = dict((external_crate['name'], int(crate_id_str)) for crate_id_str, external_crate in crate['external_crates'].items())
rdids_by_path = dict(((item_summary['crate_id'], tuple(item_summary['path']), item_summary['kind']), rdid) for rdid, item_summary in crate['paths'].items())

String_rdid = rdids_by_path[(external_crate_ids_by_name['alloc'], ('alloc', 'string', 'String'), 'struct')]
Vec_rdid = rdids_by_path[(external_crate_ids_by_name['alloc'], ('alloc', 'vec', 'Vec'), 'struct')]
Option_rdid = rdids_by_path[(external_crate_ids_by_name['core'], ('core', 'option', 'Option'), 'enum')]
cbor_Value_rdid = rdids_by_path[(external_crate_ids_by_name['sk_cbor'], ('sk_cbor', 'values', 'Value'), 'enum')]


def render_field(rdid):
    item = crate['index'][rdid]
    print('rendering field', item, file=sys.stderr)
    source = ''
    if item['docs'] is not None:
        source += '    /**\n' + re.sub(r'^', '     * ', item['docs'], flags=re.M) + '\n     */\n'
    name = item['name']
    for attr in item['attrs']:
        m = re.match(r'#\[cbor\(rename = "(\w+)"\)]', attr)
        if m is None:
            continue
        name = m.group(1)
    source += '    ' + name + ': ' + visit_rdtype(item['inner']) + ';\n'
    return source


def visit_struct(rdid):
    print('visiting struct', crate['paths'][rdid], file=sys.stderr)
    if rdid in used_types_by_rdid:
        return used_types_by_rdid[rdid].ref
    item = crate['index'][rdid]
    ref = item['name']  # %%%
    source = ''
    if item['docs'] is not None:
        source += '/**\n' + re.sub(r'^', ' * ', item['docs'], flags=re.M) + '\n */\n'
    if item['inner']['struct_type'] == 'plain':
        source += 'export interface ' + ref + ' {\n' + ''.join(render_field(field_rdid) for field_rdid in item['inner']['fields']) + '}\n'
    elif item['inner']['struct_type'] == 'tuple':
        if item['inner']['fields_stripped']:
            # fixme: we need transparent type interior
            source += 'export type ' + ref + ' = oasis.types.NotModeled; // stripped tuple type\n'
        else:
            # fixme: field names are just numbers
            source += 'export type ' + ref + ' = [' + ', '.join(visit_rdtype(crate['index'][field_rdid]['inner']) for field_rdid in item['inner']['fields']) + '];\n'
    else:
        print('unhandled struct_type', item['inner']['struct_type'], file=sys.stderr)
        source += 'export type ' + ref + ' = oasis.types.NotModeled; // unhandled struct_type ' + item['inner']['struct_type']
    used_types_by_rdid[rdid] = UsedType(ref, source)
    return ref


def render_variant(rdid):
    print('rendering variant', crate['paths'][rdid], file=sys.stderr)
    item = crate['index'][rdid]
    source = ''
    name = item['name']
    for attr in item['attrs']:
        m = re.match(r'#\[cbor\(rename = "(\w+)"\)]', attr)
        if m is None:
            continue
        name = m.group(1)
    if item['inner']['variant_kind'] == 'plain':
        if item['docs'] is not None:
            # fixme: there's no way to attach documentation to such a variant
            source += re.sub(r'^', '    // ', item['docs'], flags=re.M) + '\n'
        # fixme: may be serialized as an explicit value instead of name string
        source += '    \'' + name + '\''
    elif item['inner']['variant_kind'] == 'tuple':
        source += '    {\n'
        if item['docs'] is not None:
            source += '        /**\n' + re.sub(r'^', '         * ', item['docs'], flags=re.M) + '\n         */\n'
        if len(item['inner']['variant_inner']) == 1:
            type_source = visit_rdtype(item['inner']['variant_inner'][0])
        else:
            type_source = '[' + ', '.join(visit_rdtype(rdtype) for rdtype in item['inner']['variant_inner']) + ']'
        source += '        ' + name + ': ' + type_source + ';\n'
        source += '    }'
    elif item['inner']['variant_kind'] == 'struct':
        source += '    {\n'
        if item['docs'] is not None:
            source += '        /**\n' + re.sub(r'^', '         * ', item['docs'], flags=re.M) + '\n         */\n'
        # fixme: indentation is wrong with render_field
        type_source = '{\n' + ''.join(render_field(field_rdid) for field_rdid in item['inner']['variant_inner']) + '}'
        source += '        ' + name + ': ' + type_source + ';\n'
        source += '    }'
    else:
        print('unhandled variant kind', item['inner']['variant_kind'], file=sys.stderr)
        if item['docs'] is not None:
            source += re.sub(r'^', '    // ', item['docs'], flags=re.M) + '\n'
        source += '    oasis.types.NotModeled /* unhandled kind ' + item['inner']['variant_kind'] + ' */'
    return source


def visit_enum(rdid):
    print('visiting enum', crate['paths'][rdid], file=sys.stderr)
    if rdid in used_types_by_rdid:
        return used_types_by_rdid[rdid].ref
    item = crate['index'][rdid]
    ref = item['name']  # %%%
    source = ''
    if item['docs'] is not None:
        source += '/**\n' + re.sub(r'^', ' * ', item['docs'], flags=re.M) + '\n */\n'
    source += 'export type ' + ref + ' =\n' + ' |\n'.join(render_variant(variant_rdid) for variant_rdid in item['inner']['variants']) + ';\n'
    used_types_by_rdid[rdid] = UsedType(ref, source)
    return ref


def visit_typedef(rdid):
    print('visiting typedef', crate['paths'][rdid], file=sys.stderr)
    item = crate['index'][rdid]
    return visit_rdtype(item['inner']['type'])


def visit_resolved_path(resolved_path):
    print('visiting resolved path', resolved_path, file=sys.stderr)
    rdid = resolved_path['id']
    if rdid == String_rdid:
        return 'string'
    elif rdid == Vec_rdid:
        if resolved_path['args']['angle_bracketed']['args'][0]['type']['kind'] == 'primitive' and resolved_path['args']['angle_bracketed']['args'][0]['type']['inner'] == 'u8':
            return 'Uint8Array'
        return visit_rdtype(resolved_path['args']['angle_bracketed']['args'][0]['type']) + '[]'
    elif rdid == Option_rdid:
        return '(' + visit_rdtype(resolved_path['args']['angle_bracketed']['args'][0]['type']) + ' | null)'
    elif rdid == cbor_Value_rdid:
        return 'unknown'
    if rdid not in crate['index']:
        print('unindexed resolved path id', rdid, 'path', crate['paths'][rdid], file=sys.stderr)
        return 'oasis.types.NotModeled /* unindexed resolved path id ' + rdid + ' path ' + repr(crate['paths'][rdid]) + ' */'
    item = crate['index'][rdid]
    if item['kind'] == 'struct':
        return visit_struct(rdid)
    elif item['kind'] == 'enum':
        return visit_enum(rdid)
    elif item['kind'] == 'typedef':
        return visit_typedef(rdid)
    else:
        print('unhandled item kind', item['kind'], file=sys.stderr)
        return 'any /* unhandled item ' + rdid + ' kind ' + item['kind'] + ' */'


def visit_rdtype(rdtype):
    if rdtype['kind'] == 'resolved_path':
        return visit_resolved_path(rdtype['inner'])
    elif rdtype['kind'] == 'primitive':
        if rdtype['inner'] in {'u8', 'u16', 'u32', 'i8', 'i16', 'i32'}:
            return 'number /* ' + rdtype['inner'] + ' */'
        elif rdtype['inner'] in {'u64', 'i64'}:
            return 'oasis.types.longnum /* ' + rdtype['inner'] + ' */'
        elif rdtype['inner'] in {'u128', 'i128'}:
            return 'Uint8Array /* ' + rdtype['inner'] + ' */'
        else:
            print('unhandled primitive', rdtype['inner'], file=sys.stderr)
            return 'any /* unhandled primitive ' + rdtype['inner'] + ' */'
    elif rdtype['kind'] == 'array':
        if rdtype['inner']['type']['kind'] == 'primitive' and rdtype['inner']['type']['inner'] == 'u8':
            return 'Uint8Array'
        return visit_rdtype(rdtype['inner']['type']) + '[]'
    else:
        print('unhandled tag', rdtype['kind'], 'content', rdtype['inner'], file=sys.stderr)
        return 'any /* unhandled tag ' + rdtype['kind'] + ' content ' + repr(rdtype['inner']) + ' */'


visit_enum(rdids_by_path[(this_crate_id, ('oasis_runtime_sdk', 'modules', 'accounts', 'Event'), 'enum')])
visit_struct(rdids_by_path[(this_crate_id, ('oasis_runtime_sdk', 'types', 'transaction', 'Transaction'), 'struct')])
visit_struct(rdids_by_path[(this_crate_id, ('oasis_runtime_sdk', 'types', 'transaction', 'UnverifiedTransaction'), 'struct')])
print('\n'.join(used_type.source for used_type in sorted(used_types_by_rdid.values(), key=lambda used_type: used_type.ref)))
