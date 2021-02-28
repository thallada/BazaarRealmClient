#include <cstdarg>
#include <cstdint>
#include <cstdlib>
#include <new>
#include <cassert>


struct FFIServerError {
  uint16_t status;
  const char *title;
  const char *detail;
};

struct FFIError {
  enum class Tag : uint8_t {
    Server,
    Network,
  };

  struct Server_Body {
    FFIServerError _0;
  };

  struct Network_Body {
    const char *_0;
  };

  Tag tag;
  union {
    Server_Body server;
    Network_Body network;
  };

  static FFIError Server(const FFIServerError &_0) {
    FFIError result;
    ::new (&result.server._0) (FFIServerError)(_0);
    result.tag = Tag::Server;
    return result;
  }

  bool IsServer() const {
    return tag == Tag::Server;
  }

  const FFIServerError& AsServer() const {
    assert(IsServer());
    return server._0;
  }

  static FFIError Network(const char *const &_0) {
    FFIError result;
    ::new (&result.network._0) (const char*)(_0);
    result.tag = Tag::Network;
    return result;
  }

  bool IsNetwork() const {
    return tag == Tag::Network;
  }

  const char*const & AsNetwork() const {
    assert(IsNetwork());
    return network._0;
  }
};

template<typename T>
struct FFIResult {
  enum class Tag : uint8_t {
    Ok,
    Err,
  };

  struct Ok_Body {
    T _0;
  };

  struct Err_Body {
    FFIError _0;
  };

  Tag tag;
  union {
    Ok_Body ok;
    Err_Body err;
  };

  static FFIResult Ok(const T &_0) {
    FFIResult result;
    ::new (&result.ok._0) (T)(_0);
    result.tag = Tag::Ok;
    return result;
  }

  bool IsOk() const {
    return tag == Tag::Ok;
  }

  const T& AsOk() const {
    assert(IsOk());
    return ok._0;
  }

  static FFIResult Err(const FFIError &_0) {
    FFIResult result;
    ::new (&result.err._0) (FFIError)(_0);
    result.tag = Tag::Err;
    return result;
  }

  bool IsErr() const {
    return tag == Tag::Err;
  }

  const FFIError& AsErr() const {
    assert(IsErr());
    return err._0;
  }
};

struct RawInteriorRef {
  const char *base_mod_name;
  uint32_t base_local_form_id;
  const char *ref_mod_name;
  uint32_t ref_local_form_id;
  float position_x;
  float position_y;
  float position_z;
  float angle_x;
  float angle_y;
  float angle_z;
  uint16_t scale;
};

struct RawShelf {
  uint32_t shelf_type;
  float position_x;
  float position_y;
  float position_z;
  float angle_x;
  float angle_y;
  float angle_z;
  uint16_t scale;
  uint32_t page;
  uint32_t filter_form_type;
  bool filter_is_food;
  const char *search;
  const char *sort_on;
  bool sort_asc;
};

struct RawMerchandise {
  const char *mod_name;
  uint32_t local_form_id;
  const char *name;
  uint32_t quantity;
  uint32_t form_type;
  bool is_food;
  uint32_t price;
  const char **keywords;
  uintptr_t keywords_len;
};

struct RawMerchandiseVec {
  RawMerchandise *ptr;
  uintptr_t len;
  uintptr_t cap;
};

struct RawOwner {
  int32_t id;
  const char *name;
  int32_t mod_version;
};

struct RawShop {
  int32_t id;
  const char *name;
  const char *description;
  int32_t gold;
  const char *shop_type;
  const char **vendor_keywords;
  uintptr_t vendor_keywords_len;
  bool vendor_keywords_exclude;
};

struct RawTransaction {
  int32_t id;
  int32_t shop_id;
  const char *mod_name;
  int32_t local_form_id;
  const char *name;
  int32_t form_type;
  bool is_food;
  int32_t price;
  bool is_sell;
  int32_t quantity;
  int32_t amount;
  const char **keywords;
  uintptr_t keywords_len;
};

struct RawInteriorRefVec {
  RawInteriorRef *ptr;
  uintptr_t len;
  uintptr_t cap;
};

struct RawShelfVec {
  RawShelf *ptr;
  uintptr_t len;
  uintptr_t cap;
};

struct RawInteriorRefData {
  RawInteriorRefVec interior_ref_vec;
  RawShelfVec shelf_vec;
};

struct RawShopVec {
  RawShop *ptr;
  uintptr_t len;
  uintptr_t cap;
};

/* bad hack added by thallada. See: https://github.com/eqrion/cbindgen/issues/402 */
struct _Helper_0 {
    FFIResult<bool> _bool_result;
    FFIResult<int32_t> _int_result;
    FFIResult<RawOwner> _raw_owner_result;
    FFIResult<RawShop> _raw_shop_result;
    FFIResult<RawShopVec> _raw_shop_vec_result;
    FFIResult<RawInteriorRefData> _raw_interior_ref_data_result;
    FFIResult<RawMerchandiseVec> _raw_merchandise_vec_result;
    FFIResult<RawTransaction> _raw_transaction_result;
};

// dummy extern C block to close curly brace (did I mention this is a bad hack?)
extern "C" {
};


extern "C" {

FFIResult<int32_t> create_interior_ref_list(const char *api_url,
                                            const char *api_key,
                                            int32_t shop_id,
                                            const RawInteriorRef *raw_interior_ref_ptr,
                                            uintptr_t raw_interior_ref_len,
                                            const RawShelf *raw_shelf_ptr,
                                            uintptr_t raw_shelf_len);

FFIResult<RawMerchandiseVec> create_merchandise_list(const char *api_url,
                                                     const char *api_key,
                                                     int32_t shop_id,
                                                     const RawMerchandise *raw_merchandise_ptr,
                                                     uintptr_t raw_merchandise_len);

FFIResult<RawOwner> create_owner(const char *api_url,
                                 const char *api_key,
                                 const char *name,
                                 int32_t mod_version);

FFIResult<RawShop> create_shop(const char *api_url,
                               const char *api_key,
                               const char *name,
                               const char *description);

FFIResult<RawTransaction> create_transaction(const char *api_url,
                                             const char *api_key,
                                             RawTransaction raw_transaction);

void free_string(char *ptr);

char *generate_api_key();

FFIResult<RawInteriorRefData> get_interior_ref_list(const char *api_url,
                                                    const char *api_key,
                                                    int32_t interior_ref_list_id);

FFIResult<RawInteriorRefData> get_interior_ref_list_by_shop_id(const char *api_url,
                                                               const char *api_key,
                                                               int32_t shop_id);

FFIResult<RawMerchandiseVec> get_merchandise_list(const char *api_url,
                                                  const char *api_key,
                                                  int32_t merchandise_list_id);

FFIResult<RawMerchandiseVec> get_merchandise_list_by_shop_id(const char *api_url,
                                                             const char *api_key,
                                                             int32_t shop_id);

FFIResult<RawShop> get_shop(const char *api_url, const char *api_key, int32_t shop_id);

bool init();

FFIResult<RawShopVec> list_shops(const char *api_url, const char *api_key);

FFIResult<bool> status_check(const char *api_url);

FFIResult<int32_t> update_interior_ref_list(const char *api_url,
                                            const char *api_key,
                                            int32_t shop_id,
                                            const RawInteriorRef *raw_interior_ref_ptr,
                                            uintptr_t raw_interior_ref_len,
                                            const RawShelf *raw_shelf_ptr,
                                            uintptr_t raw_shelf_len);

FFIResult<RawMerchandiseVec> update_merchandise_list(const char *api_url,
                                                     const char *api_key,
                                                     int32_t shop_id,
                                                     const RawMerchandise *raw_merchandise_ptr,
                                                     uintptr_t raw_merchandise_len);

FFIResult<RawOwner> update_owner(const char *api_url,
                                 const char *api_key,
                                 int32_t id,
                                 const char *name,
                                 int32_t mod_version);

FFIResult<RawShop> update_shop(const char *api_url,
                               const char *api_key,
                               uint32_t id,
                               const char *name,
                               const char *description,
                               int32_t gold,
                               const char *shop_type,
                               const char **vendor_keywords,
                               uintptr_t vendor_keywords_len,
                               bool vendor_keywords_exclude);

} // extern "C"
