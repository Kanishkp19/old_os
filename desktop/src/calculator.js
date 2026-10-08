// Recursive descent over a bounded numeric grammar. No code evaluation or implicit globals.
export function calculate(source) {
  if (typeof source !== 'string' || source.length > 512) throw new Error('Expression is too long.');
  const text = source.replace(/×/g, '*').replace(/÷/g, '/');
  let pos = 0, depth = 0;
  const space = () => { while (/\s/.test(text[pos] || '') && pos < text.length) pos++; };
  const take = c => { space(); if (text[pos] === c) { pos++; return true; } return false; };
  const finite = n => { if (!Number.isFinite(n)) throw new Error('Result is outside the supported range.'); return n; };
  function atom() {
    space();
    if (++depth > 32) throw new Error('Too many brackets.');
    let value;
    if (take('(')) { value = sum(); if (!take(')')) throw new Error('Close the bracket.'); }
    else { const match = /^(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?/.exec(text.slice(pos)); if (!match) throw new Error('Enter a number.'); pos += match[0].length; value = finite(Number(match[0])); }
    depth--; while (take('%')) value /= 100; return value;
  }
  function power() { const value = atom(); return take('^') ? finite(value ** unary()) : value; }
  function unary() { if (++depth > 32) throw new Error('Expression is too complex.'); let value; if (take('+')) value = unary(); else if (take('-')) value = -unary(); else value = power(); depth--; return value; }
  function product() { let value = unary(); for (;;) { if (take('*')) value = finite(value * unary()); else if (take('/')) { const right = unary(); if (right === 0) throw new Error('Cannot divide by zero.'); value = finite(value / right); } else return value; } }
  function sum() { let value = product(); for (;;) { if (take('+')) value = finite(value + product()); else if (take('-')) value = finite(value - product()); else return value; } }
  const value = sum(); space(); if (pos !== text.length) throw new Error('Use numbers, brackets and arithmetic operators only.'); return Object.is(value, -0) ? 0 : value;
}
