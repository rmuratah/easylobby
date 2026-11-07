import { render } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import type { Position } from '../bindings/Position';
import { PositionIcon } from './PositionIcon';

describe('PositionIcon', () => {
  it('mantém um símbolo local, decorativo e distinto para cada cargo', () => {
    const positions: Position[] = ['top', 'jungle', 'middle', 'bottom', 'utility', 'fill', 'unselected'];
    const drawings = positions.map(position => {
      const { container, unmount } = render(<PositionIcon position={position} />);
      const svg = container.querySelector('svg');
      expect(svg).toHaveAttribute('aria-hidden', 'true');
      expect(svg).toHaveAttribute('viewBox', '0 0 24 24');
      const drawing = svg?.innerHTML;
      unmount();
      return drawing;
    });
    expect(new Set(drawings).size).toBe(positions.length);
  });
});
