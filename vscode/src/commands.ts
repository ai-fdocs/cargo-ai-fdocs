import * as vscode from 'vscode';
import { BinaryManager } from './binary-manager';

export async function syncAll(
    binaryManager: BinaryManager,
    outputChannel: vscode.OutputChannel,
    force: boolean = false
): Promise<void> {
    const args = ['sync'];
    if (force) {
        args.push('--force');
    }

    await vscode.window.withProgress(
        {
            location: vscode.ProgressLocation.Notification,
            title: 'AI-Docs: Syncing documentation...',
            cancellable: false,
        },
        async progress => {
            try {
                progress.report({ message: 'Running sync...' });
                const { stdout, stderr } = await binaryManager.execute(args);

                outputChannel.appendLine('=== Sync Output ===');
                outputChannel.appendLine(stdout);
                if (stderr) {
                    outputChannel.appendLine('=== Errors ===');
                    outputChannel.appendLine(stderr);
                }

                vscode.window.showInformationMessage('Documentation synced successfully!');
            } catch (error: any) {
                outputChannel.appendLine(`Sync failed: ${error.message}`);
                vscode.window.showErrorMessage(`Sync failed: ${error.message}`);
            }
        }
    );
}

export async function syncPackage(
    binaryManager: BinaryManager,
    outputChannel: vscode.OutputChannel,
    packageName: string,
    force: boolean = false
): Promise<void> {
    // Note: Current CLI doesn't support per-package sync
    // This would require CLI enhancement or we sync all with force flag
    vscode.window.showWarningMessage(
        `Per-package sync not yet supported. Running full sync instead.`
    );
    await syncAll(binaryManager, outputChannel, force);
}
