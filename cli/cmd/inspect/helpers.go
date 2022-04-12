package inspect

import (
	"context"

	"github.com/oasisprotocol/oasis-core/go/common/crypto/signature"
	"github.com/oasisprotocol/oasis-core/go/common/node"
	consensus "github.com/oasisprotocol/oasis-core/go/consensus/api"
	registry "github.com/oasisprotocol/oasis-core/go/registry/api"
)

type nodeLookup struct {
	consensus consensus.ClientBackend
	registry  registry.Backend
	nodeMap   map[signature.PublicKey]*node.Node

	height  int64
	haveAll bool
}

func (nl *nodeLookup) SetHeight(
	ctx context.Context,
	height int64,
) error {
	var force bool
	if height == consensus.HeightLatest {
		blk, err := nl.consensus.GetBlock(ctx, height)
		if err != nil {
			return err
		}
		height = blk.Height
		force = true
	}

	if nl.height != height || force {
		nl.nodeMap = make(map[signature.PublicKey]*node.Node)
		nl.haveAll = false

		// Some but not all configurations allow the GetNodes query,
		// try to get all of the nodes for the new height.
		allNodes, err := nl.registry.GetNodes(ctx, height)
		if err == nil {
			nl.haveAll = true
			for idx, node := range allNodes {
				nl.nodeMap[node.ID] = allNodes[idx]
			}
		}
	}

	nl.height = height

	return nil
}

func (nl *nodeLookup) ByID(
	ctx context.Context,
	id signature.PublicKey,
) (*node.Node, error) {
	node, ok := nl.nodeMap[id]
	if ok {
		return node, nil
	}
	if nl.haveAll {
		return nil, registry.ErrNoSuchNode
	}

	var err error
	if node, err = nl.registry.GetNode(
		ctx,
		&registry.IDQuery{
			Height: nl.height,
			ID:     id,
		},
	); err == nil {
		nl.nodeMap[id] = node
	}

	return node, err
}

func newNodeLookup(
	ctx context.Context,
	consensus consensus.ClientBackend,
	registry registry.Backend,
	height int64,
) (*nodeLookup, error) {
	nl := &nodeLookup{
		consensus: consensus,
		registry:  registry,
		nodeMap:   make(map[signature.PublicKey]*node.Node),
		height:    -1,
	}
	if err := nl.SetHeight(ctx, height); err != nil {
		return nil, err
	}
	return nl, nil
}
