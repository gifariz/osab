// Small canvas textures: round joint dots, the usual support symbols and a spring symbol.

import * as THREE from "three";

import type { SceneGlyph } from "../../scene/types";

const cache = new Map<string, THREE.Texture>();

function canvasTexture(key: string, size: number, draw: (g: CanvasRenderingContext2D) => void): THREE.Texture {
  const hit = cache.get(key);
  if (hit) return hit;
  const c = document.createElement("canvas");
  c.width = c.height = size;
  const g = c.getContext("2d")!;
  draw(g);
  const t = new THREE.CanvasTexture(c);
  t.colorSpace = THREE.SRGBColorSpace;
  cache.set(key, t);
  return t;
}

export function dotTexture(): THREE.Texture {
  return canvasTexture("dot", 64, (g) => {
    g.fillStyle = "#ffffff";
    g.beginPath();
    g.arc(32, 32, 28, 0, Math.PI * 2);
    g.fill();
  });
}

/** Support symbol, top centre = the joint. The spring symbol runs from the right edge (the joint) to a
 * hatched ground on the left. */
export function glyphTexture(kind: SceneGlyph["kind"], color: string): THREE.Texture {
  return canvasTexture(`glyph:${kind}:${color}`, 64, (g) => {
    g.strokeStyle = color;
    g.fillStyle = color;
    g.lineWidth = 4;
    const hatch = (y: number) => {
      g.beginPath();
      g.moveTo(6, y);
      g.lineTo(58, y);
      g.stroke();
      g.lineWidth = 2.5;
      for (let x = 10; x <= 58; x += 10) {
        g.beginPath();
        g.moveTo(x, y);
        g.lineTo(x - 8, y + 10);
        g.stroke();
      }
      g.lineWidth = 4;
    };
    const triangle = () => {
      g.beginPath();
      g.moveTo(32, 3);
      g.lineTo(14, 34);
      g.lineTo(50, 34);
      g.closePath();
      g.stroke();
    };
    switch (kind) {
      case "fixed":
        g.fillRect(14, 2, 36, 10);
        hatch(14);
        break;
      case "pinned":
        triangle();
        hatch(38);
        break;
      case "roller":
        triangle();
        for (const x of [22, 42]) {
          g.beginPath();
          g.arc(x, 41, 5, 0, Math.PI * 2);
          g.stroke();
        }
        hatch(50);
        break;
      case "partial":
        g.strokeRect(20, 4, 24, 24);
        g.beginPath();
        g.moveTo(20, 28);
        g.lineTo(44, 4);
        g.stroke();
        break;
      case "spring": {
        g.lineWidth = 3;
        g.beginPath();
        g.moveTo(64, 32);
        g.lineTo(52, 32);
        for (let k = 0; k < 6; k++) g.lineTo(48 - k * 6, k % 2 ? 42 : 22);
        g.lineTo(14, 32);
        g.lineTo(8, 32);
        g.stroke();
        g.beginPath();
        g.moveTo(8, 14);
        g.lineTo(8, 50);
        g.stroke();
        g.lineWidth = 2;
        for (let y = 16; y <= 50; y += 8) {
          g.beginPath();
          g.moveTo(8, y);
          g.lineTo(1, y + 7);
          g.stroke();
        }
        break;
      }
    }
  });
}
