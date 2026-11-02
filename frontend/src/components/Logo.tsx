import { BRAND } from '@/lib/brand.generated';
import React from "react";
import { Dialog, DialogContent, DialogTitle, DialogTrigger } from "./ui/dialog";
import { VisuallyHidden } from "./ui/visually-hidden";
import { About } from "./About";

interface LogoProps {
  isCollapsed: boolean;
}

/** The mark when the sidebar is collapsed, the serif wordmark when it is open. Opens About. */
const Logo = React.forwardRef<HTMLButtonElement, LogoProps>(
  ({ isCollapsed }, ref) => {
    return (
      <Dialog aria-describedby={undefined}>
        <DialogTrigger asChild>
          {isCollapsed ? (
            <button
              ref={ref}
              type="button"
              className="flex items-center justify-center mb-2 rounded-control p-1 hover:bg-surface-hover transition-colors"
              aria-label={`About ${BRAND.name}`}
            >
              {/* eslint-disable-next-line @next/next/no-img-element */}
              <img src="/icon.svg" alt="" width={36} height={36} />
            </button>
          ) : (
            <button
              ref={ref}
              type="button"
              className="w-full flex items-center gap-2 mb-2 rounded-control px-2 py-1 hover:bg-surface-hover transition-colors"
              aria-label={`About ${BRAND.name}`}
            >
              {/* eslint-disable-next-line @next/next/no-img-element */}
              <img src="/icon.svg" alt="" width={28} height={28} />
              <span className="font-serif text-2xl text-ink">{BRAND.name}</span>
            </button>
          )}
        </DialogTrigger>
        <DialogContent>
          <VisuallyHidden>
            <DialogTitle>About {BRAND.name}</DialogTitle>
          </VisuallyHidden>
          <About />
        </DialogContent>
      </Dialog>
    );
  },
);

Logo.displayName = "Logo";

export default Logo;
