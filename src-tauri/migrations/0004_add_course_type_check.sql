alter table collections
   add constraint chk_collection_type
      check ( type in ( 'series',
                        'movie',
                        'course' ) );