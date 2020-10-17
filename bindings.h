#include <cstdarg>
#include <cstdint>
#include <cstdlib>
#include <new>
#include <cassert>


struct RefRecord {
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

struct MerchRecord {
  const char *mod_name;
  uint32_t local_form_id;
  const char *name;
  uint32_t quantity;
  uint32_t form_type;
  uint8_t is_food;
  uint32_t price;
};

struct RefRecordVec {
  RefRecord *ptr;
  uintptr_t len;
  uintptr_t cap;
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
    const char *_0;
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

  static FFIResult Err(const char *const &_0) {
    FFIResult result;
    ::new (&result.err._0) (const char*)(_0);
    result.tag = Tag::Err;
    return result;
  }

  bool IsErr() const {
    return tag == Tag::Err;
  }

  const char*const & AsErr() const {
    assert(IsErr());
    return err._0;
  }
};

struct MerchRecordVec {
  MerchRecord *ptr;
  uintptr_t len;
  uintptr_t cap;
};

/* bad hack added by thallada. See: https://github.com/eqrion/cbindgen/issues/402 */
struct _Helper_0 {
    FFIResult<RefRecordVec> field;
};

struct _Helper_1 {
    FFIResult<MerchRecordVec> field;
};

// dummy extern C block to close curly brace
extern "C" {
};


extern "C" {

int32_t create_interior_ref_list(const char *api_url,
                                 const char *api_key,
                                 int32_t shop_id,
                                 const RefRecord *ref_records,
                                 uintptr_t ref_records_len);

int32_t create_merchandise_list(const char *api_url,
                                const char *api_key,
                                int32_t shop_id,
                                const MerchRecord *merch_records,
                                uintptr_t merch_records_len);

int32_t create_owner(const char *api_url,
                     const char *api_key,
                     const char *name,
                     uint32_t mod_version);

int32_t create_shop(const char *api_url,
                    const char *api_key,
                    const char *name,
                    const char *description);

void free_string(char *ptr);

char *generate_api_key();

FFIResult<RefRecordVec> get_interior_ref_list(const char *api_url,
                                              const char *api_key,
                                              int32_t interior_ref_list_id);

FFIResult<MerchRecordVec> get_merchandise_list(const char *api_url,
                                               const char *api_key,
                                               int32_t merchandise_list_id);

bool init();

bool status_check(const char *api_url);

} // extern "C"
