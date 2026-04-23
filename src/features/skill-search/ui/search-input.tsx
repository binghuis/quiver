import { forwardRef } from "react";
import { Search, X } from "lucide-react";
import { Kbd } from "@/shared/ui/kbd";

type Props = {
  query: string;
  onQueryChange: (q: string) => void;
};

export const SearchInput = forwardRef<HTMLInputElement, Props>(
  function SearchInput({ query, onQueryChange }, ref) {
    return (
      <div className="flex h-7 w-full items-center gap-2 rounded-md bg-muted/40 px-2.5 transition-colors focus-within:bg-muted">
        <Search size={12} className="shrink-0 text-muted-foreground" />
        <input
          ref={ref}
          value={query}
          onChange={(e) => onQueryChange(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Escape") {
              e.preventDefault();
              if (query) onQueryChange("");
              else (e.currentTarget as HTMLInputElement).blur();
            }
          }}
          placeholder="搜索 skill…"
          className="min-w-0 flex-1 bg-transparent text-[12.5px] outline-none placeholder:text-muted-foreground/70"
        />
        {query ? (
          <button
            onClick={() => onQueryChange("")}
            className="flex size-4 shrink-0 items-center justify-center rounded-sm text-muted-foreground hover:bg-muted hover:text-foreground"
            title="清空"
          >
            <X size={10} />
          </button>
        ) : (
          <Kbd className="shrink-0">⌘F</Kbd>
        )}
      </div>
    );
  },
);
