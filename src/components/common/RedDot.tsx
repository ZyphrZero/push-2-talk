interface RedDotProps {
  size?: 'sm' | 'md';
}

export function RedDot({ size = 'md' }: RedDotProps) {
  const sizeClass = size === 'sm' ? 'w-1.5 h-1.5' : 'w-2 h-2';

  return (
    <span
      className={`${sizeClass} bg-red-500 rounded-full animate-pulse-scale`}
      role="status"
      aria-label="有可用更新"
    />
  );
}
