import { BRAND } from '@/lib/brand.generated';
import React, { useState, useEffect } from "react";
import { invoke } from '@tauri-apps/api/core';
import { getVersion } from '@tauri-apps/api/app';
import Image from 'next/image';


export function About() {
    const [currentVersion, setCurrentVersion] = useState<string>('0.4.1');

    useEffect(() => {
        // Get current version on mount
        getVersion().then(setCurrentVersion).catch(console.error);
    }, []);

    const handleContactClick = async () => {
        try {
            await invoke('open_external_url', { url: 'https://meetily.zackriya.com/#about' });
        } catch (error) {
            console.error('Failed to open link:', error);
        }
    };

    return (
        <div className="p-4 space-y-4 h-[80vh] overflow-y-auto">
            {/* Compact Header */}
            <div className="text-center">
                <div className="mb-3">
                    <Image
                        src="icon_128x128.png"
                        alt={`${BRAND.name} logo`}
                        width={64}
                        height={64}
                        className="mx-auto"
                    />
                </div>
                                <span className="text-sm text-ink-muted"> v{currentVersion}</span>
                <p className="text-medium text-ink-muted mt-1">
                    Real-time notes and summaries that never leave your machine.
                </p>
            </div>

            {/* Features Grid - Compact */}
            <div className="space-y-3">
                <h2 className="text-base font-semibold text-ink">What makes {BRAND.name} different</h2>
                <div className="grid grid-cols-2 gap-2">
                    <div className="bg-surface rounded p-3 hover:bg-surface-hover transition-colors">
                        <h3 className="font-bold text-sm text-ink mb-1">Privacy-first</h3>
                        <p className="text-xs text-ink-muted leading-relaxed">Your data & AI processing workflow can now stay within your premise. No cloud, no leaks.</p>
                    </div>
                    <div className="bg-surface rounded p-3 hover:bg-surface-hover transition-colors">
                        <h3 className="font-bold text-sm text-ink mb-1">Use Any Model</h3>
                        <p className="text-xs text-ink-muted leading-relaxed">Prefer local open-source model? Great. Want to plug in an external API? Also fine. No lock-in.</p>
                    </div>
                    <div className="bg-surface rounded p-3 hover:bg-surface-hover transition-colors">
                        <h3 className="font-bold text-sm text-ink mb-1">Cost-Smart</h3>
                        <p className="text-xs text-ink-muted leading-relaxed">Avoid pay-per-minute bills by running models locally (or pay only for the calls you choose).</p>
                    </div>
                    <div className="bg-surface rounded p-3 hover:bg-surface-hover transition-colors">
                        <h3 className="font-bold text-sm text-ink mb-1">Works everywhere</h3>
                        <p className="text-xs text-ink-muted leading-relaxed">Google Meet, Zoom, Teams-online or offline.</p>
                    </div>
                </div>
            </div>

            {/* Coming Soon - Compact */}
            <div className="bg-accent-tint rounded p-3">
                <p className="text-s text-accent-strong">
                    <span className="font-bold">Coming soon:</span> A library of on-device AI agents-automating follow-ups, action tracking, and more.
                </p>
            </div>

            {/* CTA Section - Compact */}
            <div className="text-center space-y-2">
                <h3 className="text-medium font-semibold text-ink">Ready to push your business further?</h3>
                <p className="text-s text-ink-muted">
                    If you're planning to build privacy-first custom AI agents or a fully tailored product for your <span className="font-bold">business</span>, we can help you build it.
                </p>
                <button
                    onClick={handleContactClick}
                    className="inline-flex items-center px-4 py-2 bg-accent-strong hover:bg-accent-strong-hover text-on-accent text-sm font-medium rounded transition-colors duration-200 "
                >
                    Chat with the Zackriya team
                </button>
            </div>

            {/* Footer - Compact */}
            <div className="pt-2 border-t border-line text-center">
                <p className="text-xs text-ink-muted">
                    Built by Zackriya Solutions
                </p>
            </div>
        </div>

    )
}