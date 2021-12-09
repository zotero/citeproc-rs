import React, { useState, useCallback, useEffect, useRef } from 'react';
import { Driver, Reference } from '../../pkg';
import { Result } from 'safe-types';
import Select from 'react-select'
import hpccWasm from '@hpcc-js/wasm';

import './GraphViz.css';

export const Dot = ({ dotString }: { dotString: string }) => {
  if (dotString == null) return <></>;
  const svgRef = useRef<HTMLDivElement>();

  const renderDot = async () => {
    let svg = await hpccWasm.graphviz.layout(dotString, "svg", "dot", { wasmFolder: '/' })
    if (svgRef.current != null) {
      svgRef.current.innerHTML = svg;
    }
  };

  useEffect(() => {
    renderDot();
  }, [dotString, svgRef.current]);

  return <div ref={svgRef}></div>;
}

export const GraphViz = ({ driver, references }: { driver: Result<Driver, any>, references: Reference[] }) => {
  const ids = references.map(r => ({ value: r.id, label: r.id }));
  ids.splice(0, 0, { value: null, label: "None" });
  const [current, setCurrent] = useState<string>(null);
  const ref = references.find(r => r.id == current);
  return (
    <div>
      <div style={{ maxWidth: '400px' }} >
        <Select options={ids} onChange={({ value }) => {
          setCurrent(value);
        }} />
      </div>
      {
        ref != null
        && driver
          .map(d => d.disambiguationDfaDot(current || ""))
          .map(d => <Dot dotString={d} />)
          .unwrap_or(<></>)
      }
    </div>
  );
};

