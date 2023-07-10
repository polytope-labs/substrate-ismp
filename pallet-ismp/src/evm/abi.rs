//! Solidity rust binding
use alloy_sol_macro::sol;
sol! {
        struct PostRequest {
            // the source state machine of this request
            bytes source;
            // the destination state machine of this request
            bytes dest;
            // request nonce
            uint256 nonce;
            // Module Id of this request origin
            bytes from;
            // destination module id
            bytes to;
            // timestamp by which this request times out.
            uint256 timeoutTimestamp;
            // request body
            bytes data;
        }

        struct GetRequest {
            // the source state machine of this request
            bytes source;
            // the destination state machine of this request
            bytes dest;
            // request nonce
            uint256 nonce;
            // Module Id of this request origin
            bytes from;
            // destination has to be a contract
            bytes to;
            // timestamp by which this request times out.
            uint256 timeoutTimestamp;
            // request body
            bytes[] keys;
            // height at which to read destination state machine
            uint256 height;
        }

        struct StorageValue {
            bytes key;
            OptionValue value;
        }

       struct OptionValue {
            bytes value;
            bool some;
       }

        struct GetResponse {
            // The request that initiated this response
            GetRequest request;
            // storage values for get response
            StorageValue[] values;
        }

        struct PostResponse {
            // The request that initiated this response
            PostRequest request;
            // bytes for post response
            bytes response;
        }

        // An object for dispatching post requests to the IsmpDispatcher
        struct DispatchPost {
            // bytes representation of the destination chain
            bytes destChain;
            // the destination module
            bytes to;
            // the request body
            bytes data;
            // Timeout
            uint256 timeoutTimestamp;
        }

        // An object for dispatching post requests to the IsmpDispatcher
        struct DispatchGet {
            // bytes representation of the destination chain
            bytes destChain;
            // Height
            uint256 height;
            // the request body
            bytes[] keys;
            // Timeout
            uint256 timeoutTimestamp;
        }


        function OnAccept(PostRequest memory request) external;
        function OnPostResponse(PostResponse memory response) external;
        function OnGetResponse(GetResponse memory response) external;
        function OnPostTimeout(PostRequest memory request) external;
        function OnGetTimeout(GetRequest memory request) external;

}
