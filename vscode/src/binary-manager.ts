import * as vscode from 'vscode';
import { exec } from 'child_process';
import { promisify } from 'util';
import * as path from 'path';
import * as fs from 'fs';

const execAsync = promisify(exec);

export interface BinaryInfo {
    path: string;
    version: string;
    type: 'npm' | 'rust';
}

export class BinaryManager {
    private binaryInfo: BinaryInfo | null = null;
    private outputChannel: vscode.OutputChannel;

    constructor(outputChannel: vscode.OutputChannel) {
        this.outputChannel = outputChannel;
    }

    /**
     * Detect ai-fdocs binary in PATH or custom location
     */
    async detectBinary(): Promise<BinaryInfo | null> {
        // Check custom path from settings
        const config = vscode.workspace.getConfiguration('ai-fdocs');
        const customPath = config.get<string>('binaryPath');

        if (customPath && customPath.trim() !== '') {
            const info = await this.checkBinaryPath(customPath);
            if (info) {
                this.binaryInfo = info;
                return info;
            }
        }

        // Try NPM version first (ai-fdocs)
        const npmInfo = await this.checkBinaryPath('ai-fdocs');
        if (npmInfo) {
            this.binaryInfo = npmInfo;
            return npmInfo;
        }

        // Try Rust version (cargo-ai-fdocs)
        const rustInfo = await this.checkBinaryPath('cargo-ai-fdocs');
        if (rustInfo) {
            this.binaryInfo = rustInfo;
            return rustInfo;
        }

        // Try cargo ai-fdocs (subcommand style)
        const cargoSubInfo = await this.checkCargoSubcommand();
        if (cargoSubInfo) {
            this.binaryInfo = cargoSubInfo;
            return cargoSubInfo;
        }

        return null;
    }

    /**
     * Check if a binary path is valid and get its version
     */
    private async checkBinaryPath(binaryPath: string): Promise<BinaryInfo | null> {
        try {
            const { stdout } = await execAsync(`"${binaryPath}" --version`, {
                timeout: 5000,
            });

            const version = this.parseVersion(stdout);
            const type = binaryPath.includes('cargo') ? 'rust' : 'npm';

            this.outputChannel.appendLine(`Found binary: ${binaryPath} (${type}) v${version}`);

            return {
                path: binaryPath,
                version,
                type,
            };
        } catch (error) {
            // Binary not found or error executing
            return null;
        }
    }

    /**
     * Check for cargo ai-fdocs subcommand
     */
    private async checkCargoSubcommand(): Promise<BinaryInfo | null> {
        try {
            const { stdout } = await execAsync('cargo ai-fdocs --version', {
                timeout: 5000,
            });

            const version = this.parseVersion(stdout);

            this.outputChannel.appendLine(`Found cargo subcommand: cargo ai-fdocs v${version}`);

            return {
                path: 'cargo ai-fdocs',
                version,
                type: 'rust',
            };
        } catch (error) {
            return null;
        }
    }

    /**
     * Parse version from command output
     */
    private parseVersion(output: string): string {
        const match = output.match(/(\d+\.\d+\.\d+)/);
        return match ? match[1] : 'unknown';
    }

    /**
     * Execute binary with arguments
     */
    async execute(args: string[], cwd?: string): Promise<{ stdout: string; stderr: string }> {
        if (!this.binaryInfo) {
            throw new Error('Binary not detected. Please install ai-fdocs or configure binary path.');
        }

        const command = `"${this.binaryInfo.path}" ${args.join(' ')}`;
        this.outputChannel.appendLine(`Executing: ${command}`);

        try {
            const result = await execAsync(command, {
                cwd: cwd || vscode.workspace.workspaceFolders?.[0]?.uri.fsPath,
                timeout: 120000, // 2 minutes timeout
                maxBuffer: 10 * 1024 * 1024, // 10MB buffer
            });

            this.outputChannel.appendLine(`Success: ${result.stdout.substring(0, 500)}`);
            return result;
        } catch (error: any) {
            this.outputChannel.appendLine(`Error: ${error.message}`);
            throw error;
        }
    }

    /**
     * Get binary version
     */
    async getVersion(): Promise<string> {
        if (!this.binaryInfo) {
            const info = await this.detectBinary();
            if (!info) {
                throw new Error('Binary not found');
            }
        }
        return this.binaryInfo!.version;
    }

    /**
     * Check if binary is available
     */
    async isAvailable(): Promise<boolean> {
        const info = await this.detectBinary();
        return info !== null;
    }

    /**
     * Get current binary info
     */
    getBinaryInfo(): BinaryInfo | null {
        return this.binaryInfo;
    }

    /**
     * Show installation instructions
     */
    async showInstallationInstructions(): Promise<void> {
        const choice = await vscode.window.showErrorMessage(
            'ai-fdocs binary not found. Please install it to use this extension.',
            'Install via NPM',
            'Install via Cargo',
            'Configure Path'
        );

        if (choice === 'Install via NPM') {
            vscode.env.openExternal(vscode.Uri.parse('https://www.npmjs.com/package/ai-fdocs'));
        } else if (choice === 'Install via Cargo') {
            vscode.env.openExternal(vscode.Uri.parse('https://crates.io/crates/cargo-ai-fdocs'));
        } else if (choice === 'Configure Path') {
            vscode.commands.executeCommand('workbench.action.openSettings', 'ai-fdocs.binaryPath');
        }
    }
}
