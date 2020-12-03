import React, { useState, useCallback, useEffect, useRef } from 'react';
import { Driver, Reference } from '../../pkg';
import { Result } from 'safe-types';
import Select from 'react-select'
import dot from "graphlib-dot";
import dagreD3 from 'dagre-d3';
import { select } from 'd3-selection';
import { zoomIdentity } from 'd3-zoom';
import { curveBasis, curveBundle } from 'd3-shape';

import './GraphViz.css';

export const Dot = ({ dotString }: { dotString: string }) => {
    if (dotString == null) return <></>;
    const svgRef = useRef();
    const svgGroupRef = useRef();

    let render = dagreD3.render();

    useEffect(() => {
        let svg = select(svgRef.current);
        let group = select(svgGroupRef.current);

        const g = dot.read(dotString);

        g.nodes().forEach(v => {
            var node = g.node(v);
            // Round the corners of the nodes
            node.rx = node.ry = 5;
            let label: string = node.label;
            if (label.match(/Accepting/)) {
                node.class = "type-Accepting";
            } else {
                node.class = "";
            }
        });

        g.edges().forEach(e => {
            var edge = g.edge(e);
            let label: string = edge.label;
            if (label.match(/Output/)) {
            } else {
                edge.labelStyle = "font-weight: bold;"
            }
            // edge.curve = curveBasis;
            edge.curve = curveBundle.beta(0.8);
        });

        render(group, g);

        let cur = svgRef.current;
        if (cur != null) {
            let {height, width} = cur.getBBox();
            let {height: gHeight, width: gWidth} = g.graph();
            let transX = width - gWidth;
            let transY = height - gHeight;
            svg.attr("height", height);
            svg.attr("width", width + 30);
            group.attr("transform", zoomIdentity.translate(transX, transY))
        }
        return () => {
            group.selectAll("*").remove();
        }
    }, [dotString, svgRef.current]);

    return <svg className='dagre-d3' ref={svgRef} width={900} height={800}>
        <g ref={svgGroupRef}/>
    </svg>;
}

export const GraphViz = ({driver, references}: { driver: Result<Driver, any>, references: Reference[] }) => {
    const ids = references.map(r => ({value: r.id, label: r.id}));
    ids.splice(0, 0, { value: null, label: "None" });
    const [current, setCurrent] = useState<string>(null);
    const ref = references.find(r => r.id == current);
    return (
        <div>
            <div style={{maxWidth: '400px'}} >
                <Select options={ids} onChange={({value}) => {
                    setCurrent(value);
                } } />
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

