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
      const drawing = svg?.innerHTML;
      unmount();
      return drawing;
    });
    expect(new Set(drawings).size).toBe(positions.length);
  });

  it('usa o desenho oficial do client, pintado com a cor do texto', () => {
    const lanes: Position[] = ['top', 'jungle', 'middle', 'bottom', 'utility'];
    for (const lane of lanes) {
      const { container, unmount } = render(<PositionIcon position={lane} size={20} />);
      const svg = container.querySelector('svg');
      expect(svg).toHaveAttribute('viewBox', '0 0 34 34');
      expect(svg).toHaveAttribute('fill', 'currentColor');
      expect(svg).toHaveAttribute('width', '20');
      unmount();
    }
  });

  it('apaga pela metade a parte secundária, como no client', () => {
    const { container } = render(<PositionIcon position="top" />);
    const paths = container.querySelectorAll('path');
    expect(paths).toHaveLength(2);
    expect(paths[0]).toHaveAttribute('opacity', '0.5');
    expect(paths[1]).not.toHaveAttribute('opacity');
  });
});
