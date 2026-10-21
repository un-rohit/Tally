const fs = require('fs');
const crypto = require('crypto');

const filePath = 'C:\\Program Files\\TallyPrime\\tally.exe';
const buf = fs.readFileSync(filePath);

// Basic hashes
const md5 = crypto.createHash('md5').update(buf).digest('hex');
const sha1 = crypto.createHash('sha1').update(buf).digest('hex');
const sha256 = crypto.createHash('sha256').update(buf).digest('hex');

// DOS Header
const peOffset = buf.readInt32LE(0x3C);
const magic = buf.toString('ascii', 0, 2);

// NT Headers
const peSig = buf.toString('ascii', peOffset, peOffset + 4);
const machine = buf.readUInt16LE(peOffset + 4);
const numSections = buf.readUInt16LE(peOffset + 6);
const timeDateStamp = buf.readUInt32LE(peOffset + 8);
const optHeaderSize = buf.readUInt16LE(peOffset + 20);
const characteristics = buf.readUInt16LE(peOffset + 22);

// Optional Header (PE32+)
const optOffset = peOffset + 24;
const optMagic = buf.readUInt16LE(optOffset); // 0x20b = PE32+
const majorLinker = buf.readUInt8(optOffset + 2);
const minorLinker = buf.readUInt8(optOffset + 3);
const sizeOfCode = buf.readUInt32LE(optOffset + 4);
const sizeOfInitData = buf.readUInt32LE(optOffset + 8);
const sizeOfUninitData = buf.readUInt32LE(optOffset + 12);
const entryPointRva = buf.readUInt32LE(optOffset + 16);
const baseOfCode = buf.readUInt32LE(optOffset + 20);
const imageBase = buf.readBigUInt64LE(optOffset + 24);
const sectionAlignment = buf.readUInt32LE(optOffset + 32);
const fileAlignment = buf.readUInt32LE(optOffset + 36);
const majorOS = buf.readUInt16LE(optOffset + 40);
const minorOS = buf.readUInt16LE(optOffset + 42);
const majorImage = buf.readUInt16LE(optOffset + 44);
const minorImage = buf.readUInt16LE(optOffset + 46);
const majorSubsystem = buf.readUInt16LE(optOffset + 48);
const minorSubsystem = buf.readUInt16LE(optOffset + 50);
const sizeOfImage = buf.readUInt32LE(optOffset + 56);
const sizeOfHeaders = buf.readUInt32LE(optOffset + 60);
const checksum = buf.readUInt32LE(optOffset + 64);
const subsystem = buf.readUInt16LE(optOffset + 68);
const dllCharacteristics = buf.readUInt16LE(optOffset + 70);

// Data Directories (16 entries, 8 bytes each)
const dataDirOffset = optOffset + 112;
const dirNames = [
    'Export Table', 'Import Table', 'Resource Table', 'Exception Table',
    'Certificate Table', 'Base Relocation Table', 'Debug', 'Architecture',
    'Global Ptr', 'TLS Table', 'Load Config Table', 'Bound Import',
    'IAT', 'Delay Import Descriptor', 'CLR Runtime Header', 'Reserved'
];
const dataDirectories = [];
for (let i = 0; i < 16; i++) {
    const rva = buf.readUInt32LE(dataDirOffset + (i * 8));
    const size = buf.readUInt32LE(dataDirOffset + (i * 8) + 4);
    dataDirectories.push({ name: dirNames[i], rva, size });
}

// Section Headers
const sectionTableOffset = optOffset + optHeaderSize;
const sections = [];
for (let i = 0; i < numSections; i++) {
    const secOffset = sectionTableOffset + (i * 40);
    const name = buf.toString('ascii', secOffset, secOffset + 8).replace(/\0/g, '');
    const virtSize = buf.readUInt32LE(secOffset + 8);
    const virtAddr = buf.readUInt32LE(secOffset + 12);
    const rawSize = buf.readUInt32LE(secOffset + 16);
    const rawOffset = buf.readUInt32LE(secOffset + 20);
    const secCharacteristics = buf.readUInt32LE(secOffset + 36);

    // Calculate section entropy
    let entropy = 0;
    if (rawSize > 0 && rawOffset + rawSize <= buf.length) {
        const counts = new Uint32Array(256);
        for (let j = 0; j < rawSize; j++) counts[buf[rawOffset + j]]++;
        for (let j = 0; j < 256; j++) {
            if (counts[j] > 0) {
                const p = counts[j] / rawSize;
                entropy -= p * Math.log2(p);
            }
        }
    }

    sections.push({
        name,
        virtSize,
        virtAddr,
        rawSize,
        rawOffset,
        characteristics: '0x' + secCharacteristics.toString(16),
        entropy: Number(entropy.toFixed(3))
    });
}

function rvaToOffset(rva) {
    for (const s of sections) {
        if (rva >= s.virtAddr && rva < s.virtAddr + s.virtSize) {
            return s.rawOffset + (rva - s.virtAddr);
        }
    }
    return null;
}

// Imports
const imports = {};
const importDir = dataDirectories[1];
if (importDir.rva > 0) {
    let impDescOffset = rvaToOffset(importDir.rva);
    while (impDescOffset && impDescOffset + 20 <= buf.length) {
        const nameRva = buf.readUInt32LE(impDescOffset + 12);
        if (nameRva === 0) break;
        const firstThunkRva = buf.readUInt32LE(impDescOffset + 16);
        const originalFirstThunkRva = buf.readUInt32LE(impDescOffset);
        const thunkRva = (originalFirstThunkRva !== 0) ? originalFirstThunkRva : firstThunkRva;

        const dllNameOffset = rvaToOffset(nameRva);
        let dllName = '';
        if (dllNameOffset) {
            let p = dllNameOffset;
            while (p < buf.length && buf[p] !== 0) {
                dllName += String.fromCharCode(buf[p++]);
            }
        }

        const funcs = [];
        let thunkOffset = rvaToOffset(thunkRva);
        while (thunkOffset && thunkOffset + 8 <= buf.length) {
            const val = buf.readBigUInt64LE(thunkOffset);
            if (val === 0n) break;
            const isOrdinal = (val & 0x8000000000000000n) !== 0n;
            if (isOrdinal) {
                funcs.push({ ordinal: Number(val & 0xFFFFn) });
            } else {
                const hintRva = Number(val & 0xFFFFFFFFn);
                const hintOffset = rvaToOffset(hintRva);
                if (hintOffset) {
                    const hint = buf.readUInt16LE(hintOffset);
                    let funcName = '';
                    let p = hintOffset + 2;
                    while (p < buf.length && buf[p] !== 0) {
                        funcName += String.fromCharCode(buf[p++]);
                    }
                    funcs.push({ hint, name: funcName });
                }
            }
            thunkOffset += 8;
        }

        imports[dllName] = funcs;
        impDescOffset += 20;
    }
}

// Exports
const exports = [];
const exportDir = dataDirectories[0];
if (exportDir.rva > 0) {
    const expOffset = rvaToOffset(exportDir.rva);
    if (expOffset) {
        const numFunctions = buf.readUInt32LE(expOffset + 20);
        const numNames = buf.readUInt32LE(expOffset + 24);
        const functionsRva = buf.readUInt32LE(expOffset + 28);
        const namesRva = buf.readUInt32LE(expOffset + 32);
        const ordinalsRva = buf.readUInt32LE(expOffset + 36);

        const namesOffset = rvaToOffset(namesRva);
        const ordinalsOffset = rvaToOffset(ordinalsRva);
        const functionsOffset = rvaToOffset(functionsRva);

        for (let i = 0; i < numNames; i++) {
            const nameEntryRva = buf.readUInt32LE(namesOffset + (i * 4));
            const nameOffset = rvaToOffset(nameEntryRva);
            let name = '';
            let p = nameOffset;
            while (p < buf.length && buf[p] !== 0) name += String.fromCharCode(buf[p++]);

            const ordinal = buf.readUInt16LE(ordinalsOffset + (i * 2));
            const funcRva = buf.readUInt32LE(functionsOffset + (ordinal * 4));
            exports.push({ name, ordinal, rva: '0x' + funcRva.toString(16) });
        }
    }
}

// Manifest extraction
let manifestXml = null;
const manifestDir = dataDirectories[2]; // Resource Table
// We know from inspect_pe_resources that resource index 9 (RT_MANIFEST) is at offset 18474356, length 932
const manifestOffset = 18474356;
if (manifestOffset + 932 <= buf.length) {
    manifestXml = buf.toString('utf8', manifestOffset, manifestOffset + 932);
}

// Debug Directory (PDB path / RSDS)
let debugInfo = null;
const debugDir = dataDirectories[6];
if (debugDir.rva > 0 && debugDir.size > 0) {
    const debugOffset = rvaToOffset(debugDir.rva);
    if (debugOffset && debugOffset + 28 <= buf.length) {
        const type = buf.readUInt32LE(debugOffset + 12);
        const sizeOfData = buf.readUInt32LE(debugOffset + 16);
        const addressOfRawData = buf.readUInt32LE(debugOffset + 20);
        const pointerToRawData = buf.readUInt32LE(debugOffset + 24);
        if (type === 2 && pointerToRawData + sizeOfData <= buf.length) { // CodeView
            const cvSig = buf.toString('ascii', pointerToRawData, pointerToRawData + 4);
            let pdbPath = '';
            if (cvSig === 'RSDS') {
                const guid = buf.slice(pointerToRawData + 4, pointerToRawData + 20).toString('hex');
                const age = buf.readUInt32LE(pointerToRawData + 20);
                let p = pointerToRawData + 24;
                while (p < pointerToRawData + sizeOfData && buf[p] !== 0) pdbPath += String.fromCharCode(buf[p++]);
                debugInfo = { format: 'RSDS', guid, age, pdbPath };
            }
        }
    }
}

const analysis = {
    file: {
        path: filePath,
        sizeBytes: buf.length,
        hashes: { md5, sha1, sha256 }
    },
    peHeader: {
        format: 'PE32+ (64-bit)',
        machine: '0x' + machine.toString(16) + ' (AMD64 / x86_64)',
        linkerVersion: `${majorLinker}.${minorLinker}`,
        subsystem: subsystem === 2 ? 'Windows GUI' : (subsystem === 3 ? 'Windows CUI' : subsystem),
        entryPointRva: '0x' + entryPointRva.toString(16),
        entryPointFileOffset: '0x' + (rvaToOffset(entryPointRva) || 0).toString(16),
        imageBase: '0x' + imageBase.toString(16),
        sizeOfImage: '0x' + sizeOfImage.toString(16),
        sectionAlignment: '0x' + sectionAlignment.toString(16),
        fileAlignment: '0x' + fileAlignment.toString(16),
        dllCharacteristics: '0x' + dllCharacteristics.toString(16)
    },
    sections,
    dataDirectories: dataDirectories.filter(d => d.size > 0),
    exports: {
        count: exports.length,
        items: exports
    },
    imports: {
        dllCount: Object.keys(imports).length,
        modules: Object.keys(imports).map(dll => ({
            name: dll,
            count: imports[dll].length,
            sampleFunctions: imports[dll].slice(0, 10).map(f => f.name || `Ordinal_${f.ordinal}`)
        }))
    },
    debugInfo,
    manifest: manifestXml
};

fs.writeFileSync('d:\\Tally\\tally_analysis\\tally_exe_deep_analysis.json', JSON.stringify(analysis, null, 2));
console.log('Analysis complete. Written to tally_exe_deep_analysis.json');
