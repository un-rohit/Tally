const fs = require('fs');
const path = require('path');

function extractStrings(filePath, minLen = 4) {
    if (!fs.existsSync(filePath)) return [];
    const buf = fs.readFileSync(filePath);
    const strings = [];
    let start = -1;
    for (let i = 0; i < buf.length; i++) {
        const b = buf[i];
        if (b >= 32 && b <= 126) {
            if (start === -1) start = i;
        } else {
            if (start !== -1 && (i - start) >= minLen) {
                strings.push({
                    offset: start,
                    str: buf.toString('utf8', start, i)
                });
            }
            start = -1;
        }
    }
    return strings;
}

const resStrings = extractStrings('C:\\Program Files\\TallyPrime\\tallyres.dat');
console.log(`Extracted ${resStrings.length} strings from tallyres.dat`);

const queries = [
    'Gateway of Tally',
    'ENVELOPE',
    'TALLYMESSAGE',
    '<VOUCHER',
    '<LEDGER',
    '<COMPANY',
    'ODBC',
    '9000',
    '9999',
    'Balance Sheet',
    'Profit & Loss',
    'Trial Balance',
    'Day Book',
    'Chart of Accounts',
    'VCHTYPE',
    'VoucherTypeName',
    'secDB',
    'Tally Gateway'
];

const results = {};
for (const q of queries) {
    const matches = resStrings.filter(s => s.str.includes(q)).slice(0, 5);
    results[q] = matches.map(m => `[0x${m.offset.toString(16)}] ${m.str.substring(0, 120)}`);
}

fs.writeFileSync('d:\\Tally\\tally_analysis\\summary_matches.json', JSON.stringify(results, null, 2));
console.log('Saved summary matches.');
