import * as vscode from 'vscode';
import { BinaryManager } from './binary-manager';
import { ProjectDetector, ProjectInfo } from './project-detector';
import { DependencyTreeProvider, TreeItemType } from './dependency-tree-provider';
import { syncAll } from './commands';

let outputChannel: vscode.OutputChannel;
let binaryManager: BinaryManager;
let projectDetector: ProjectDetector;
let treeDataProvider: DependencyTreeProvider;
let statusBarItem: vscode.StatusBarItem;
let currentProject: ProjectInfo | null = null;


export async function activate(context: vscode.ExtensionContext) {
    console.log('AI Fresh Docs extension is now active');

    // Create output channel
    outputChannel = vscode.window.createOutputChannel('AI Fresh Docs');
    context.subscriptions.push(outputChannel);

    // Initialize binary manager
    binaryManager = new BinaryManager(outputChannel);

    // Check if binary is available
    const isAvailable = await binaryManager.isAvailable();
    if (!isAvailable) {
        outputChannel.appendLine('ai-fdocs binary not found');
        await binaryManager.showInstallationInstructions();
        return; // Don't activate further if binary not found
    }

    const binaryInfo = binaryManager.getBinaryInfo();
    outputChannel.appendLine(
        `Using ai-fdocs binary: ${binaryInfo?.path} (${binaryInfo?.type}) v${binaryInfo?.version}`
    );

    // Initialize project detector
    projectDetector = new ProjectDetector();
    const projects = await projectDetector.detectProjects();

    if (projects.length === 0) {
        outputChannel.appendLine('No supported projects found in workspace');
        vscode.window.showInformationMessage(
            'No Rust or Node.js projects detected. AI Fresh Docs extension will remain inactive.'
        );
        return;
    }

    // Use first project for now (multi-project support can be added later)
    currentProject = projects[0];
    outputChannel.appendLine(
        `Detected ${ProjectDetector.getProjectTypeName(currentProject.type)} project at ${currentProject.rootPath}`
    );

    // Initialize tree data provider
    treeDataProvider = new DependencyTreeProvider(binaryManager, outputChannel);
    treeDataProvider.setProjectRoot(currentProject.rootPath);

    // Register tree view
    const treeView = vscode.window.createTreeView('ai-fdocs-dependencies', {
        treeDataProvider: treeDataProvider,
    });
    context.subscriptions.push(treeView);

    // Initial refresh
    await treeDataProvider.refresh();

    // Register commands
    context.subscriptions.push(
        vscode.commands.registerCommand('ai-fdocs.sync', async () => {
            await syncAll(binaryManager, outputChannel, false);
            await treeDataProvider.refresh();
            updateStatusBar();
        })
    );

    context.subscriptions.push(
        vscode.commands.registerCommand('ai-fdocs.forceSync', async () => {
            await syncAll(binaryManager, outputChannel, true);
            await treeDataProvider.refresh();
            updateStatusBar();
        })
    );

    context.subscriptions.push(
        vscode.commands.registerCommand('ai-fdocs.refresh', async () => {
            await treeDataProvider.refresh();
            updateStatusBar();
        })
    );

    context.subscriptions.push(
        vscode.commands.registerCommand('ai-fdocs.init', async () => {
            await vscode.window.withProgress(
                {
                    location: vscode.ProgressLocation.Notification,
                    title: 'AI-Docs: Initializing configuration...',
                    cancellable: false,
                },
                async () => {
                    try {
                        await binaryManager.execute(['init']);
                        vscode.window.showInformationMessage('ai-fdocs.toml created successfully!');
                        await treeDataProvider.refresh();
                    } catch (error: any) {
                        vscode.window.showErrorMessage(`Init failed: ${error.message}`);
                    }
                }
            );
        })
    );

    context.subscriptions.push(
        vscode.commands.registerCommand('ai-fdocs.prune', async () => {
            const confirm = await vscode.window.showWarningMessage(
                'This will remove outdated documentation. Continue?',
                'Yes',
                'No'
            );

            if (confirm !== 'Yes') {
                return;
            }

            await vscode.window.withProgress(
                {
                    location: vscode.ProgressLocation.Notification,
                    title: 'AI-Docs: Pruning documentation...',
                    cancellable: false,
                },
                async () => {
                    try {
                        await binaryManager.execute(['prune']);
                        vscode.window.showInformationMessage('Documentation pruned successfully!');
                        await treeDataProvider.refresh();
                    } catch (error: any) {
                        vscode.window.showErrorMessage(`Prune failed: ${error.message}`);
                    }
                }
            );
        })
    );

    context.subscriptions.push(
        vscode.commands.registerCommand('ai-fdocs.check', async () => {
            try {
                const { stdout } = await binaryManager.execute(['check', '--format', 'json']);
                outputChannel.appendLine('=== Check Output ===');
                outputChannel.appendLine(stdout);
                outputChannel.show();

                const result = JSON.parse(stdout);
                const summary = result.summary;

                if (summary.missing > 0 || summary.outdated > 0 || summary.corrupted > 0) {
                    vscode.window.showWarningMessage(
                        `Documentation check: ${summary.missing} missing, ${summary.outdated} outdated, ${summary.corrupted} corrupted`
                    );
                } else {
                    vscode.window.showInformationMessage('All documentation is up to date!');
                }
            } catch (error: any) {
                vscode.window.showErrorMessage(`Check failed: ${error.message}`);
            }
        })
    );

    // Create status bar item
    statusBarItem = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Left, 100);
    statusBarItem.command = 'ai-fdocs.refresh';
    context.subscriptions.push(statusBarItem);
    updateStatusBar();
    statusBarItem.show();

    // Setup file watchers
    const config = vscode.workspace.getConfiguration('ai-fdocs');
    const autoSync = config.get<boolean>('autoSync');

    if (autoSync) {
        projectDetector.setupWatchers(async () => {
            outputChannel.appendLine('Lockfile or config changed, auto-syncing...');
            await syncAll(binaryManager, outputChannel, false);
            await treeDataProvider.refresh();
            updateStatusBar();
        });
    }

    // Watch for config changes
    context.subscriptions.push(
        vscode.workspace.onDidChangeConfiguration(e => {
            if (e.affectsConfiguration('ai-fdocs')) {
                outputChannel.appendLine('Configuration changed, refreshing...');
                treeDataProvider.refresh();
            }
        })
    );

    vscode.window.showInformationMessage('AI Fresh Docs extension activated!');
}

function updateStatusBar() {
    if (!currentProject || !statusBarItem) {
        return;
    }

    const emoji = ProjectDetector.getProjectEmoji(currentProject.type);
    const typeName = ProjectDetector.getProjectTypeName(currentProject.type);

    statusBarItem.text = `${emoji} ${typeName} - AI Docs`;
    statusBarItem.tooltip = 'Click to refresh documentation status';
}

export function deactivate() {
    if (projectDetector) {
        projectDetector.disposeWatchers();
    }
    outputChannel.appendLine('AI Fresh Docs extension deactivated');
}
