const path = require('path');
const fs = require('fs');
const vscode = require('vscode');
const { LanguageClient, TransportKind } = require('vscode-languageclient/node');

let client;

function findBinary() {
    const config = vscode.workspace.getConfiguration('judas');
    const customPath = config.get('lsp.path');
    if (customPath && customPath.trim().length > 0) {
        return customPath.trim();
    }

    // Check cargo release/debug build directories in workspace
    const workspaceFolders = vscode.workspace.workspaceFolders;
    if (workspaceFolders) {
        for (const folder of workspaceFolders) {
            const root = folder.uri.fsPath;
            const releaseLsp = path.join(root, 'target', 'release', 'judas-lsp');
            if (fs.existsSync(releaseLsp)) return releaseLsp;
            const debugLsp = path.join(root, 'target', 'debug', 'judas-lsp');
            if (fs.existsSync(debugLsp)) return debugLsp;
            const releaseJudas = path.join(root, 'target', 'release', 'judas');
            if (fs.existsSync(releaseJudas)) return [releaseJudas, ['lsp']];
            const debugJudas = path.join(root, 'target', 'debug', 'judas');
            if (fs.existsSync(debugJudas)) return [debugJudas, ['lsp']];
        }
    }

    // Check ~/.cargo/bin
    const home = process.env.HOME || '';
    if (home) {
        const cargoLsp = path.join(home, '.cargo', 'bin', 'judas-lsp');
        if (fs.existsSync(cargoLsp)) return cargoLsp;
        const cargoJudas = path.join(home, '.cargo', 'bin', 'judas');
        if (fs.existsSync(cargoJudas)) return [cargoJudas, ['lsp']];
    }

    // Default to PATH
    return 'judas-lsp';
}

function activate(context) {
    const binInfo = findBinary();
    let command = 'judas-lsp';
    let args = [];

    if (Array.isArray(binInfo)) {
        command = binInfo[0];
        args = binInfo[1];
    } else {
        command = binInfo;
    }

    const serverOptions = {
        run: { command, args, transport: TransportKind.stdio },
        debug: { command, args, transport: TransportKind.stdio }
    };

    const clientOptions = {
        documentSelector: [{ scheme: 'file', language: 'judas' }],
        synchronize: {
            fileEvents: vscode.workspace.createFileSystemWatcher('**/*.jd')
        }
    };

    client = new LanguageClient(
        'judasLanguageServer',
        'Judas Language Server',
        serverOptions,
        clientOptions
    );

    client.start();
}

function deactivate() {
    if (!client) {
        return undefined;
    }
    return client.stop();
}

module.exports = {
    activate,
    deactivate
};
