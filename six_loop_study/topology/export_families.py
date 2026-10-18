#!/usr/bin/env python3
"""Export offline maximal parents using native HEPKit basis completion.

All physical denominators have common unit mass, q²−1. Native completion
selects six independent auxiliaries; RustRed receives ordinary generic TOML.
"""
import json
import argparse
from pathlib import Path
from symbolica import S, E
from symbolica.community import hepkit as hep


def momentum_text(row):
    terms=[]
    for i, value in enumerate(row):
        if value:
            coefficient='' if abs(value)==1 else f'{abs(value)}*'
            terms.append(('+' if value>0 else '-')+coefficient+f'k{i+1}')
    return ''.join(terms).lstrip('+') or '0'


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--directory',type=Path,default=Path(__file__).resolve().parent)
    directory=parser.parse_args().directory
    census=json.loads((directory/'classified_census.json').read_text())
    cover=json.loads((directory/'maximal_parent_cover.json').read_text())
    contraction_path=directory/'contraction_census.json'
    contractions=json.loads(contraction_path.read_text())if contraction_path.exists()else None
    if contractions is not None and not contractions['complete']:
        raise RuntimeError('refuse to export partial contraction representatives')
    if not cover['complete']:
        raise RuntimeError('refuse incomplete maximal-parent cover')
    raw={g['id']:g for g in json.loads((directory/'cubic_graphs.json').read_text())['graphs']}
    loops=census['loops']; count=loops*(loops+1)//2
    momenta=[S(f'k{i+1}') for i in range(loops)]
    kin=hep.Kinematics(dimension=S('d'),momenta=momenta)
    def denominator(row):
        q=sum((value*k for value,k in zip(row,momenta)),E('0'))
        return kin.scalar_product(q,q)-1
    candidates=[[int(i==j) for i in range(loops)]for j in range(loops)]
    candidates += [[int(k==i)-int(k==j) for k in range(loops)]
                   for i in range(loops) for j in range(i+1,loops)]
    candidate_expressions=[denominator(row) for row in candidates]
    (directory/'families').mkdir(exist_ok=True)
    (directory/'dot').mkdir(exist_ok=True)
    entries=[]
    for parent in census['representatives']:
        if parent['id'] not in cover['maximal_parents']:
            continue
        physical=parent['momenta']
        family=hep.IntegralFamily(momenta,[],[denominator(q) for q in physical],kinematics=kin)
        if not family.is_independent:
            raise RuntimeError('physical denominator dependence requires explicit treatment')
        complete=family.complete(candidates=candidate_expressions)
        if not complete.is_complete or complete.rank!=count:
            raise RuntimeError('native auxiliary completion failed')
        rows=list(physical)
        for auxiliary in complete.denominators[len(physical):]:
            choices=[i for i,value in enumerate(candidate_expressions) if value==auxiliary]
            if len(choices)!=1:
                raise RuntimeError('native completion selected a noncandidate auxiliary')
            rows.append(candidates[choices[0]])
        assert len(rows)==count
        name=parent['id'].replace('-','_')
        document=['schema="rustred.project.toml.v1"','[family]',f'name="{name}"',
                  'loop_momenta='+json.dumps([f'k{i+1}'for i in range(loops)]),
                  'external_momenta=[]','dimension="d"']
        for i,row in enumerate(rows):
            document.extend(['[[family.denominators]]',f'id="D{i+1}"',
                             f'expression="({momentum_text(row)})^2-1"'])
        document += ['[target]','powers='+json.dumps([1]*len(physical)+[0]*(count-len(physical)))]
        input_path=f'families/{parent["id"]}.toml'
        dot_path=f'dot/{parent["id"]}.dot'
        (directory/input_path).write_text('\n'.join(document)+'\n')
        graph=raw[parent['representative']]
        (directory/dot_path).write_text((directory/graph['dot']).read_text())
        entry=dict(id=parent['id'],input=input_path,dot=dot_path,loop_count=loops,
                   coordinate_count=count,physical_edge_count=len(physical),
                   physical_propagators=list(range(len(physical))),
                   auxiliary_slots=list(range(len(physical),count)),momenta=rows,
                   native_basis_rank=complete.rank,common_mass_squared=1,
                   denominator_convention='q^2-1',representative_graph=graph['id'],
                   coordinate_edge_ids=[group[0]for group in parent['edge_groups']],
                   coordinate_edge_signs=[group[0]for group in parent['edge_group_signs']],
                   canonical_matroid=parent['canonical_matroid'])
        if contractions is not None:
            representatives=[r for r in contractions['representatives']
                             if r['representative_parent']==parent['id']]
            entry['selected_sectors']=[''.join('1'if i in set(r['representative_slots'])else'0'
                                              for i in range(count))for r in representatives]
            entry['selected_topology_classes']=[dict(id=r['id'],classification=r['classification'],
                                                    physical_edge_count=r['physical_edge_count'])
                                                for r in representatives]
            if not representatives or len(set(entry['selected_sectors']))!=len(representatives):
                raise RuntimeError('invalid representative sector assignment')
        entries.append(entry)
    result=dict(schema='six-loop-study.topologies.v1',loop_count=loops,
                coordinate_count=count,parent_count=len(entries),entries=entries,
                generation_scope=('one selected sector for every graph-matroid class'
                                  if contractions is not None else
                                  'each maximal physical root and its full nonzero sector downset'),
                selected_sector_count=(sum(len(e['selected_sectors'])for e in entries)
                                       if contractions is not None else None),
                selected_sector_classes=(contractions['classes_by_kind']if contractions is not None else None),
                selected_generation_is_not_assembled_full_downset_closure=True,
                cross_parent_subsector_work_deduplicated=False,
                topology_census='classified_census.json',coverage_witnesses='maximal_parent_cover.json',
                contraction_census=('contraction_census.json'if contractions is not None else None),
                dot_semantics='native FeynKit phi3 DOT; masses normalized to one in TOML',
                ibp_generation_performed=False,closure_claim=False)
    (directory/'manifest.json').write_text(json.dumps(result,indent=2)+'\n')
    print(json.dumps(dict(parents=len(entries),coordinates=count,
                         physical_edges=3*(loops-1),native_completion='HEPKit IntegralFamily.complete'),indent=2))


if __name__=='__main__':
    main()
