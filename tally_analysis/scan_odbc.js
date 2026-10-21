const fs = require('fs');

const b = fs.readFileSync('C:\\Program Files\\TallyPrime\\tallywin.dat');
let s = '';
const strings = [];
for (let i = 0; i < b.length; i++) {
    if (b[i] >= 32 && b[i] <= 126) s += String.fromCharCode(b[i]);
    else {
        if (s.length >= 4) strings.push({offset: i - s.length, str: s});
        s = '';
    }
}
console.log('Total ASCII strings in tallywin.dat:', strings.length);

// Also look for UTF-16LE strings
const utf16Strings = [];
let u16 = '';
let u16Start = -1;
for (let i = 0; i < b.length - 1; i += 2) {
    const code = b[i] | (b[i+1] << 8);
    if (code >= 32 && code <= 126) {
        if (u16Start === -1) u16Start = i;
        u16 += String.fromCharCode(code);
    } else {
        if (u16.length >= 4) utf16Strings.push({offset: u16Start, str: u16});
        u16 = '';
        u16Start = -1;
    }
}
console.log('Total UTF-16 strings in tallywin.dat:', utf16Strings.length);

const allStrings = [...strings, ...utf16Strings];
const keywords = [
    'SELECT', 'Tally', 'http://', 'PORT', '9000', 'COMPANY', 'LEDGER', 'VOUCHER',
    'XML', 'ENVELOPE', 'REQUEST', 'RESPONSE', 'SQLState', 'HY000', 'DSN', 'Driver'
];

const found = {};
for (const k of keywords) {
    found[k] = allStrings
        .filter(x => x.str.toUpperCase().includes(k.toUpperCase()))
        .slice(0, 10)
        .map(x => `[0x${x.offset.toString(16)}] ${x.str}`);
}

fs.writeFileSync('d:\\Tally\\tally_analysis\\odbc_strings.json', JSON.stringify(found, null, 2));
console.log('Saved odbc_strings.json');
