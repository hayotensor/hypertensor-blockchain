// SPDX-License-Identifier: MIT
pragma solidity 0.8.28;

import "@openzeppelin/contracts/token/ERC20/extensions/ERC20Permit.sol";
import "@openzeppelin/contracts/token/ERC721/ERC721.sol";
import "@openzeppelin/contracts/token/ERC721/IERC721Receiver.sol";

contract PermitToken is ERC20Permit {
    constructor() ERC20("Integration Token", "TEST") ERC20Permit("Integration Token") {
        _mint(msg.sender, 1_000_000 ether);
    }
}

contract TestNFT is ERC721 {
    constructor() ERC721("Integration NFT", "NFT") {
        _mint(msg.sender, 1);
        _mint(msg.sender, 2);
    }
}

contract NFTReceiver is IERC721Receiver {
    address public operator;
    address public from;
    uint256 public tokenId;
    bytes public data;

    function onERC721Received(address op, address sender, uint256 id, bytes calldata payload)
        external returns (bytes4)
    {
        operator = op;
        from = sender;
        tokenId = id;
        data = payload;
        return IERC721Receiver.onERC721Received.selector;
    }
}

// Valid contract code, deliberately no ERC721 receiver interface.
contract NonReceiver {
    function ping() external pure returns (uint256) { return 42; }
}
