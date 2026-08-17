import type { ReactNode } from "react";

/** Lightweight markdown → safe React nodes (no HTML passthrough). */
export function MarkdownBody({ source }: { source: string }) {
  const blocks = source.replace(/\r\n/g, "\n").split(/\n{2,}/);
  return (
    <div className="md-body">
      {blocks.map((block, i) => {
        const trimmed = block.trim();
        if (!trimmed) return null;
        if (/^#{1,3}\s/.test(trimmed)) {
          const level = trimmed.match(/^#+/)?.[0].length ?? 1;
          const text = trimmed.replace(/^#{1,3}\s+/, "");
          if (level === 1) return <h3 key={i}>{inline(text)}</h3>;
          if (level === 2) return <h4 key={i}>{inline(text)}</h4>;
          return <h5 key={i}>{inline(text)}</h5>;
        }
        if (/^[-*]\s/m.test(trimmed) && trimmed.split("\n").every((l) => /^[-*]\s|^$/.test(l))) {
          return (
            <ul key={i}>
              {trimmed.split("\n").map((line, j) => {
                const item = line.replace(/^[-*]\s+/, "").trim();
                if (!item) return null;
                return <li key={j}>{inline(item)}</li>;
              })}
            </ul>
          );
        }
        if (trimmed.startsWith("```")) {
          const code = trimmed.replace(/^```\w*\n?/, "").replace(/\n?```$/, "");
          return (
            <pre key={i}>
              <code>{code}</code>
            </pre>
          );
        }
        return (
          <p key={i}>
            {trimmed.split("\n").map((line, j, arr) => (
              <span key={j}>
                {inline(line)}
                {j < arr.length - 1 ? <br /> : null}
              </span>
            ))}
          </p>
        );
      })}
    </div>
  );
}

function inline(text: string): ReactNode[] {
  const parts: ReactNode[] = [];
  const re =
    /(\*\*[^*]+\*\*|\*[^*]+\*|`[^`]+`|\[[^\]]+\]\((https?:\/\/[^)\s]+)\))/g;
  let last = 0;
  let m: RegExpExecArray | null;
  let key = 0;
  while ((m = re.exec(text))) {
    if (m.index > last) parts.push(text.slice(last, m.index));
    const token = m[0];
    if (token.startsWith("**")) {
      parts.push(<strong key={key++}>{token.slice(2, -2)}</strong>);
    } else if (token.startsWith("*")) {
      parts.push(<em key={key++}>{token.slice(1, -1)}</em>);
    } else if (token.startsWith("`")) {
      parts.push(<code key={key++}>{token.slice(1, -1)}</code>);
    } else {
      const link = token.match(/\[([^\]]+)\]\((https?:\/\/[^)\s]+)\)/);
      if (link) {
        parts.push(
          <a key={key++} href={link[2]} target="_blank" rel="noreferrer">
            {link[1]}
          </a>,
        );
      }
    }
    last = m.index + token.length;
  }
  if (last < text.length) parts.push(text.slice(last));
  return parts;
}
