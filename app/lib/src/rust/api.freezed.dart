// GENERATED CODE - DO NOT MODIFY BY HAND
// coverage:ignore-file
// ignore_for_file: type=lint, type=warning, deprecated_member_use, deprecated_member_use_from_same_package
// ignore_for_file: unused_element, deprecated_member_use, deprecated_member_use_from_same_package, use_function_type_syntax_for_parameters, unnecessary_const, avoid_init_to_null, invalid_override_different_default_values_named, prefer_expression_function_bodies, annotate_overrides, invalid_annotation_target, unnecessary_question_mark

part of 'api.dart';

// **************************************************************************
// FreezedGenerator
// **************************************************************************

// GENERATED CODE - DO NOT MODIFY BY HAND
// dart format off
T _$identity<T>(T value) => value;
/// @nodoc
mixin _$AttachmentStatus {





@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is AttachmentStatus);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'AttachmentStatus()';
}


}

/// @nodoc
class $AttachmentStatusCopyWith<$Res>  {
$AttachmentStatusCopyWith(AttachmentStatus _, $Res Function(AttachmentStatus) __);
}


/// Adds pattern-matching-related methods to [AttachmentStatus].
extension AttachmentStatusPatterns on AttachmentStatus {
/// A variant of `map` that fallback to returning `orElse`.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case _:
///     return orElse();
/// }
/// ```

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( AttachmentStatus_Sending value)?  sending,TResult Function( AttachmentStatus_Receiving value)?  receiving,TResult Function( AttachmentStatus_Ready value)?  ready,required TResult orElse(),}){
final _that = this;
switch (_that) {
case AttachmentStatus_Sending() when sending != null:
return sending(_that);case AttachmentStatus_Receiving() when receiving != null:
return receiving(_that);case AttachmentStatus_Ready() when ready != null:
return ready(_that);case _:
  return orElse();

}
}
/// A `switch`-like method, using callbacks.
///
/// Callbacks receives the raw object, upcasted.
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case final Subclass2 value:
///     return ...;
/// }
/// ```

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( AttachmentStatus_Sending value)  sending,required TResult Function( AttachmentStatus_Receiving value)  receiving,required TResult Function( AttachmentStatus_Ready value)  ready,}){
final _that = this;
switch (_that) {
case AttachmentStatus_Sending():
return sending(_that);case AttachmentStatus_Receiving():
return receiving(_that);case AttachmentStatus_Ready():
return ready(_that);}
}
/// A variant of `map` that fallback to returning `null`.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case _:
///     return null;
/// }
/// ```

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( AttachmentStatus_Sending value)?  sending,TResult? Function( AttachmentStatus_Receiving value)?  receiving,TResult? Function( AttachmentStatus_Ready value)?  ready,}){
final _that = this;
switch (_that) {
case AttachmentStatus_Sending() when sending != null:
return sending(_that);case AttachmentStatus_Receiving() when receiving != null:
return receiving(_that);case AttachmentStatus_Ready() when ready != null:
return ready(_that);case _:
  return null;

}
}
/// A variant of `when` that fallback to an `orElse` callback.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case _:
///     return orElse();
/// }
/// ```

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function( int sent,  int total)?  sending,TResult Function( int have,  int total)?  receiving,TResult Function()?  ready,required TResult orElse(),}) {final _that = this;
switch (_that) {
case AttachmentStatus_Sending() when sending != null:
return sending(_that.sent,_that.total);case AttachmentStatus_Receiving() when receiving != null:
return receiving(_that.have,_that.total);case AttachmentStatus_Ready() when ready != null:
return ready();case _:
  return orElse();

}
}
/// A `switch`-like method, using callbacks.
///
/// As opposed to `map`, this offers destructuring.
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case Subclass2(:final field2):
///     return ...;
/// }
/// ```

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function( int sent,  int total)  sending,required TResult Function( int have,  int total)  receiving,required TResult Function()  ready,}) {final _that = this;
switch (_that) {
case AttachmentStatus_Sending():
return sending(_that.sent,_that.total);case AttachmentStatus_Receiving():
return receiving(_that.have,_that.total);case AttachmentStatus_Ready():
return ready();}
}
/// A variant of `when` that fallback to returning `null`
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case _:
///     return null;
/// }
/// ```

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function( int sent,  int total)?  sending,TResult? Function( int have,  int total)?  receiving,TResult? Function()?  ready,}) {final _that = this;
switch (_that) {
case AttachmentStatus_Sending() when sending != null:
return sending(_that.sent,_that.total);case AttachmentStatus_Receiving() when receiving != null:
return receiving(_that.have,_that.total);case AttachmentStatus_Ready() when ready != null:
return ready();case _:
  return null;

}
}

}

/// @nodoc


class AttachmentStatus_Sending extends AttachmentStatus {
  const AttachmentStatus_Sending({required this.sent, required this.total}): super._();
  

/// Parts confirmed.
 final  int sent;
/// All parts.
 final  int total;

/// Create a copy of AttachmentStatus
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$AttachmentStatus_SendingCopyWith<AttachmentStatus_Sending> get copyWith => _$AttachmentStatus_SendingCopyWithImpl<AttachmentStatus_Sending>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is AttachmentStatus_Sending&&(identical(other.sent, sent) || other.sent == sent)&&(identical(other.total, total) || other.total == total));
}


@override
int get hashCode {
    return Object.hash(runtimeType,sent,total);
}

@override
String toString() {
    return 'AttachmentStatus.sending(sent: $sent, total: $total)';
}


}

/// @nodoc
abstract mixin class $AttachmentStatus_SendingCopyWith<$Res> implements $AttachmentStatusCopyWith<$Res> {
  factory $AttachmentStatus_SendingCopyWith(AttachmentStatus_Sending value, $Res Function(AttachmentStatus_Sending) _then) = _$AttachmentStatus_SendingCopyWithImpl;
@useResult
$Res call({
 int sent, int total
});




}
/// @nodoc
class _$AttachmentStatus_SendingCopyWithImpl<$Res>
    implements $AttachmentStatus_SendingCopyWith<$Res> {
  _$AttachmentStatus_SendingCopyWithImpl(this._self, this._then);

  final AttachmentStatus_Sending _self;
  final $Res Function(AttachmentStatus_Sending) _then;

/// Create a copy of AttachmentStatus
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? sent = null,Object? total = null,}) {
  return _then(AttachmentStatus_Sending(
sent: null == sent ? _self.sent : sent // ignore: cast_nullable_to_non_nullable
as int,total: null == total ? _self.total : total // ignore: cast_nullable_to_non_nullable
as int,
  ));
}


}

/// @nodoc


class AttachmentStatus_Receiving extends AttachmentStatus {
  const AttachmentStatus_Receiving({required this.have, required this.total}): super._();
  

/// Parts here.
 final  int have;
/// All parts.
 final  int total;

/// Create a copy of AttachmentStatus
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$AttachmentStatus_ReceivingCopyWith<AttachmentStatus_Receiving> get copyWith => _$AttachmentStatus_ReceivingCopyWithImpl<AttachmentStatus_Receiving>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is AttachmentStatus_Receiving&&(identical(other.have, have) || other.have == have)&&(identical(other.total, total) || other.total == total));
}


@override
int get hashCode {
    return Object.hash(runtimeType,have,total);
}

@override
String toString() {
    return 'AttachmentStatus.receiving(have: $have, total: $total)';
}


}

/// @nodoc
abstract mixin class $AttachmentStatus_ReceivingCopyWith<$Res> implements $AttachmentStatusCopyWith<$Res> {
  factory $AttachmentStatus_ReceivingCopyWith(AttachmentStatus_Receiving value, $Res Function(AttachmentStatus_Receiving) _then) = _$AttachmentStatus_ReceivingCopyWithImpl;
@useResult
$Res call({
 int have, int total
});




}
/// @nodoc
class _$AttachmentStatus_ReceivingCopyWithImpl<$Res>
    implements $AttachmentStatus_ReceivingCopyWith<$Res> {
  _$AttachmentStatus_ReceivingCopyWithImpl(this._self, this._then);

  final AttachmentStatus_Receiving _self;
  final $Res Function(AttachmentStatus_Receiving) _then;

/// Create a copy of AttachmentStatus
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? have = null,Object? total = null,}) {
  return _then(AttachmentStatus_Receiving(
have: null == have ? _self.have : have // ignore: cast_nullable_to_non_nullable
as int,total: null == total ? _self.total : total // ignore: cast_nullable_to_non_nullable
as int,
  ));
}


}

/// @nodoc


class AttachmentStatus_Ready extends AttachmentStatus {
  const AttachmentStatus_Ready(): super._();
  






@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is AttachmentStatus_Ready);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'AttachmentStatus.ready()';
}


}




/// @nodoc
mixin _$BleCommandItem {

/// Plugin link id.
 BigInt get link;
/// Create a copy of BleCommandItem
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$BleCommandItemCopyWith<BleCommandItem> get copyWith => _$BleCommandItemCopyWithImpl<BleCommandItem>(this as BleCommandItem, _$identity);



@override
bool operator ==(Object other) {
  final _this = this as BleCommandItem;
  return identical(this, other) || (other.runtimeType == runtimeType&&other is BleCommandItem&&(identical(other.link, _this.link) || other.link == _this.link));
}


@override
int get hashCode {
  final _this = this as BleCommandItem;
  return Object.hash(runtimeType,_this.link);
}

@override
String toString() {
  final _this = this as BleCommandItem;
  return 'BleCommandItem(link: ${_this.link})';
}


}

/// @nodoc
abstract mixin class $BleCommandItemCopyWith<$Res>  {
  factory $BleCommandItemCopyWith(BleCommandItem value, $Res Function(BleCommandItem) _then) = _$BleCommandItemCopyWithImpl;
@useResult
$Res call({
 BigInt link
});




}
/// @nodoc
class _$BleCommandItemCopyWithImpl<$Res>
    implements $BleCommandItemCopyWith<$Res> {
  _$BleCommandItemCopyWithImpl(this._self, this._then);

  final BleCommandItem _self;
  final $Res Function(BleCommandItem) _then;

/// Create a copy of BleCommandItem
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') @override $Res call({Object? link = null,}) {
  return _then(_self.copyWith(
link: null == link ? _self.link : link // ignore: cast_nullable_to_non_nullable
as BigInt,
  ));
}

}


/// Adds pattern-matching-related methods to [BleCommandItem].
extension BleCommandItemPatterns on BleCommandItem {
/// A variant of `map` that fallback to returning `orElse`.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case _:
///     return orElse();
/// }
/// ```

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( BleCommandItem_Send value)?  send,TResult Function( BleCommandItem_Close value)?  close,required TResult orElse(),}){
final _that = this;
switch (_that) {
case BleCommandItem_Send() when send != null:
return send(_that);case BleCommandItem_Close() when close != null:
return close(_that);case _:
  return orElse();

}
}
/// A `switch`-like method, using callbacks.
///
/// Callbacks receives the raw object, upcasted.
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case final Subclass2 value:
///     return ...;
/// }
/// ```

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( BleCommandItem_Send value)  send,required TResult Function( BleCommandItem_Close value)  close,}){
final _that = this;
switch (_that) {
case BleCommandItem_Send():
return send(_that);case BleCommandItem_Close():
return close(_that);}
}
/// A variant of `map` that fallback to returning `null`.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case _:
///     return null;
/// }
/// ```

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( BleCommandItem_Send value)?  send,TResult? Function( BleCommandItem_Close value)?  close,}){
final _that = this;
switch (_that) {
case BleCommandItem_Send() when send != null:
return send(_that);case BleCommandItem_Close() when close != null:
return close(_that);case _:
  return null;

}
}
/// A variant of `when` that fallback to an `orElse` callback.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case _:
///     return orElse();
/// }
/// ```

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function( BigInt link,  BigInt op,  Uint8List bytes)?  send,TResult Function( BigInt link)?  close,required TResult orElse(),}) {final _that = this;
switch (_that) {
case BleCommandItem_Send() when send != null:
return send(_that.link,_that.op,_that.bytes);case BleCommandItem_Close() when close != null:
return close(_that.link);case _:
  return orElse();

}
}
/// A `switch`-like method, using callbacks.
///
/// As opposed to `map`, this offers destructuring.
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case Subclass2(:final field2):
///     return ...;
/// }
/// ```

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function( BigInt link,  BigInt op,  Uint8List bytes)  send,required TResult Function( BigInt link)  close,}) {final _that = this;
switch (_that) {
case BleCommandItem_Send():
return send(_that.link,_that.op,_that.bytes);case BleCommandItem_Close():
return close(_that.link);}
}
/// A variant of `when` that fallback to returning `null`
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case _:
///     return null;
/// }
/// ```

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function( BigInt link,  BigInt op,  Uint8List bytes)?  send,TResult? Function( BigInt link)?  close,}) {final _that = this;
switch (_that) {
case BleCommandItem_Send() when send != null:
return send(_that.link,_that.op,_that.bytes);case BleCommandItem_Close() when close != null:
return close(_that.link);case _:
  return null;

}
}

}

/// @nodoc


class BleCommandItem_Send extends BleCommandItem {
  const BleCommandItem_Send({required this.link, required this.op, required this.bytes}): super._();
  

/// Plugin link id.
@override final  BigInt link;
/// Write id.
 final  BigInt op;
/// Stream bytes.
 final  Uint8List bytes;

/// Create a copy of BleCommandItem
/// with the given fields replaced by the non-null parameter values.
@override @JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$BleCommandItem_SendCopyWith<BleCommandItem_Send> get copyWith => _$BleCommandItem_SendCopyWithImpl<BleCommandItem_Send>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is BleCommandItem_Send&&(identical(other.link, link) || other.link == link)&&(identical(other.op, op) || other.op == op)&&const DeepCollectionEquality().equals(other.bytes, bytes));
}


@override
int get hashCode {
    return Object.hash(runtimeType,link,op,const DeepCollectionEquality().hash(bytes));
}

@override
String toString() {
    return 'BleCommandItem.send(link: $link, op: $op, bytes: $bytes)';
}


}

/// @nodoc
abstract mixin class $BleCommandItem_SendCopyWith<$Res> implements $BleCommandItemCopyWith<$Res> {
  factory $BleCommandItem_SendCopyWith(BleCommandItem_Send value, $Res Function(BleCommandItem_Send) _then) = _$BleCommandItem_SendCopyWithImpl;
@override @useResult
$Res call({
 BigInt link, BigInt op, Uint8List bytes
});




}
/// @nodoc
class _$BleCommandItem_SendCopyWithImpl<$Res>
    implements $BleCommandItem_SendCopyWith<$Res> {
  _$BleCommandItem_SendCopyWithImpl(this._self, this._then);

  final BleCommandItem_Send _self;
  final $Res Function(BleCommandItem_Send) _then;

/// Create a copy of BleCommandItem
/// with the given fields replaced by the non-null parameter values.
@override @pragma('vm:prefer-inline') $Res call({Object? link = null,Object? op = null,Object? bytes = null,}) {
  return _then(BleCommandItem_Send(
link: null == link ? _self.link : link // ignore: cast_nullable_to_non_nullable
as BigInt,op: null == op ? _self.op : op // ignore: cast_nullable_to_non_nullable
as BigInt,bytes: null == bytes ? _self.bytes : bytes // ignore: cast_nullable_to_non_nullable
as Uint8List,
  ));
}


}

/// @nodoc


class BleCommandItem_Close extends BleCommandItem {
  const BleCommandItem_Close({required this.link}): super._();
  

/// Plugin link id.
@override final  BigInt link;

/// Create a copy of BleCommandItem
/// with the given fields replaced by the non-null parameter values.
@override @JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$BleCommandItem_CloseCopyWith<BleCommandItem_Close> get copyWith => _$BleCommandItem_CloseCopyWithImpl<BleCommandItem_Close>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is BleCommandItem_Close&&(identical(other.link, link) || other.link == link));
}


@override
int get hashCode {
    return Object.hash(runtimeType,link);
}

@override
String toString() {
    return 'BleCommandItem.close(link: $link)';
}


}

/// @nodoc
abstract mixin class $BleCommandItem_CloseCopyWith<$Res> implements $BleCommandItemCopyWith<$Res> {
  factory $BleCommandItem_CloseCopyWith(BleCommandItem_Close value, $Res Function(BleCommandItem_Close) _then) = _$BleCommandItem_CloseCopyWithImpl;
@override @useResult
$Res call({
 BigInt link
});




}
/// @nodoc
class _$BleCommandItem_CloseCopyWithImpl<$Res>
    implements $BleCommandItem_CloseCopyWith<$Res> {
  _$BleCommandItem_CloseCopyWithImpl(this._self, this._then);

  final BleCommandItem_Close _self;
  final $Res Function(BleCommandItem_Close) _then;

/// Create a copy of BleCommandItem
/// with the given fields replaced by the non-null parameter values.
@override @pragma('vm:prefer-inline') $Res call({Object? link = null,}) {
  return _then(BleCommandItem_Close(
link: null == link ? _self.link : link // ignore: cast_nullable_to_non_nullable
as BigInt,
  ));
}


}

/// @nodoc
mixin _$CallSignalItem {

/// Hex call id.
 String get callId;
/// Create a copy of CallSignalItem
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$CallSignalItemCopyWith<CallSignalItem> get copyWith => _$CallSignalItemCopyWithImpl<CallSignalItem>(this as CallSignalItem, _$identity);



@override
bool operator ==(Object other) {
  final _this = this as CallSignalItem;
  return identical(this, other) || (other.runtimeType == runtimeType&&other is CallSignalItem&&(identical(other.callId, _this.callId) || other.callId == _this.callId));
}


@override
int get hashCode {
  final _this = this as CallSignalItem;
  return Object.hash(runtimeType,_this.callId);
}

@override
String toString() {
  final _this = this as CallSignalItem;
  return 'CallSignalItem(callId: ${_this.callId})';
}


}

/// @nodoc
abstract mixin class $CallSignalItemCopyWith<$Res>  {
  factory $CallSignalItemCopyWith(CallSignalItem value, $Res Function(CallSignalItem) _then) = _$CallSignalItemCopyWithImpl;
@useResult
$Res call({
 String callId
});




}
/// @nodoc
class _$CallSignalItemCopyWithImpl<$Res>
    implements $CallSignalItemCopyWith<$Res> {
  _$CallSignalItemCopyWithImpl(this._self, this._then);

  final CallSignalItem _self;
  final $Res Function(CallSignalItem) _then;

/// Create a copy of CallSignalItem
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') @override $Res call({Object? callId = null,}) {
  return _then(_self.copyWith(
callId: null == callId ? _self.callId : callId // ignore: cast_nullable_to_non_nullable
as String,
  ));
}

}


/// Adds pattern-matching-related methods to [CallSignalItem].
extension CallSignalItemPatterns on CallSignalItem {
/// A variant of `map` that fallback to returning `orElse`.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case _:
///     return orElse();
/// }
/// ```

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( CallSignalItem_Offer value)?  offer,TResult Function( CallSignalItem_Answer value)?  answer,TResult Function( CallSignalItem_Candidates value)?  candidates,TResult Function( CallSignalItem_End value)?  end,required TResult orElse(),}){
final _that = this;
switch (_that) {
case CallSignalItem_Offer() when offer != null:
return offer(_that);case CallSignalItem_Answer() when answer != null:
return answer(_that);case CallSignalItem_Candidates() when candidates != null:
return candidates(_that);case CallSignalItem_End() when end != null:
return end(_that);case _:
  return orElse();

}
}
/// A `switch`-like method, using callbacks.
///
/// Callbacks receives the raw object, upcasted.
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case final Subclass2 value:
///     return ...;
/// }
/// ```

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( CallSignalItem_Offer value)  offer,required TResult Function( CallSignalItem_Answer value)  answer,required TResult Function( CallSignalItem_Candidates value)  candidates,required TResult Function( CallSignalItem_End value)  end,}){
final _that = this;
switch (_that) {
case CallSignalItem_Offer():
return offer(_that);case CallSignalItem_Answer():
return answer(_that);case CallSignalItem_Candidates():
return candidates(_that);case CallSignalItem_End():
return end(_that);}
}
/// A variant of `map` that fallback to returning `null`.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case _:
///     return null;
/// }
/// ```

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( CallSignalItem_Offer value)?  offer,TResult? Function( CallSignalItem_Answer value)?  answer,TResult? Function( CallSignalItem_Candidates value)?  candidates,TResult? Function( CallSignalItem_End value)?  end,}){
final _that = this;
switch (_that) {
case CallSignalItem_Offer() when offer != null:
return offer(_that);case CallSignalItem_Answer() when answer != null:
return answer(_that);case CallSignalItem_Candidates() when candidates != null:
return candidates(_that);case CallSignalItem_End() when end != null:
return end(_that);case _:
  return null;

}
}
/// A variant of `when` that fallback to an `orElse` callback.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case _:
///     return orElse();
/// }
/// ```

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function( String callId,  String sdp)?  offer,TResult Function( String callId,  String sdp)?  answer,TResult Function( String callId,  List<String> candidates)?  candidates,TResult Function( String callId,  CallEndReason reason)?  end,required TResult orElse(),}) {final _that = this;
switch (_that) {
case CallSignalItem_Offer() when offer != null:
return offer(_that.callId,_that.sdp);case CallSignalItem_Answer() when answer != null:
return answer(_that.callId,_that.sdp);case CallSignalItem_Candidates() when candidates != null:
return candidates(_that.callId,_that.candidates);case CallSignalItem_End() when end != null:
return end(_that.callId,_that.reason);case _:
  return orElse();

}
}
/// A `switch`-like method, using callbacks.
///
/// As opposed to `map`, this offers destructuring.
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case Subclass2(:final field2):
///     return ...;
/// }
/// ```

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function( String callId,  String sdp)  offer,required TResult Function( String callId,  String sdp)  answer,required TResult Function( String callId,  List<String> candidates)  candidates,required TResult Function( String callId,  CallEndReason reason)  end,}) {final _that = this;
switch (_that) {
case CallSignalItem_Offer():
return offer(_that.callId,_that.sdp);case CallSignalItem_Answer():
return answer(_that.callId,_that.sdp);case CallSignalItem_Candidates():
return candidates(_that.callId,_that.candidates);case CallSignalItem_End():
return end(_that.callId,_that.reason);}
}
/// A variant of `when` that fallback to returning `null`
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case _:
///     return null;
/// }
/// ```

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function( String callId,  String sdp)?  offer,TResult? Function( String callId,  String sdp)?  answer,TResult? Function( String callId,  List<String> candidates)?  candidates,TResult? Function( String callId,  CallEndReason reason)?  end,}) {final _that = this;
switch (_that) {
case CallSignalItem_Offer() when offer != null:
return offer(_that.callId,_that.sdp);case CallSignalItem_Answer() when answer != null:
return answer(_that.callId,_that.sdp);case CallSignalItem_Candidates() when candidates != null:
return candidates(_that.callId,_that.candidates);case CallSignalItem_End() when end != null:
return end(_that.callId,_that.reason);case _:
  return null;

}
}

}

/// @nodoc


class CallSignalItem_Offer extends CallSignalItem {
  const CallSignalItem_Offer({required this.callId, required this.sdp}): super._();
  

/// Hex call id.
@override final  String callId;
/// SDP.
 final  String sdp;

/// Create a copy of CallSignalItem
/// with the given fields replaced by the non-null parameter values.
@override @JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$CallSignalItem_OfferCopyWith<CallSignalItem_Offer> get copyWith => _$CallSignalItem_OfferCopyWithImpl<CallSignalItem_Offer>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is CallSignalItem_Offer&&(identical(other.callId, callId) || other.callId == callId)&&(identical(other.sdp, sdp) || other.sdp == sdp));
}


@override
int get hashCode {
    return Object.hash(runtimeType,callId,sdp);
}

@override
String toString() {
    return 'CallSignalItem.offer(callId: $callId, sdp: $sdp)';
}


}

/// @nodoc
abstract mixin class $CallSignalItem_OfferCopyWith<$Res> implements $CallSignalItemCopyWith<$Res> {
  factory $CallSignalItem_OfferCopyWith(CallSignalItem_Offer value, $Res Function(CallSignalItem_Offer) _then) = _$CallSignalItem_OfferCopyWithImpl;
@override @useResult
$Res call({
 String callId, String sdp
});




}
/// @nodoc
class _$CallSignalItem_OfferCopyWithImpl<$Res>
    implements $CallSignalItem_OfferCopyWith<$Res> {
  _$CallSignalItem_OfferCopyWithImpl(this._self, this._then);

  final CallSignalItem_Offer _self;
  final $Res Function(CallSignalItem_Offer) _then;

/// Create a copy of CallSignalItem
/// with the given fields replaced by the non-null parameter values.
@override @pragma('vm:prefer-inline') $Res call({Object? callId = null,Object? sdp = null,}) {
  return _then(CallSignalItem_Offer(
callId: null == callId ? _self.callId : callId // ignore: cast_nullable_to_non_nullable
as String,sdp: null == sdp ? _self.sdp : sdp // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class CallSignalItem_Answer extends CallSignalItem {
  const CallSignalItem_Answer({required this.callId, required this.sdp}): super._();
  

/// Hex call id.
@override final  String callId;
/// SDP.
 final  String sdp;

/// Create a copy of CallSignalItem
/// with the given fields replaced by the non-null parameter values.
@override @JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$CallSignalItem_AnswerCopyWith<CallSignalItem_Answer> get copyWith => _$CallSignalItem_AnswerCopyWithImpl<CallSignalItem_Answer>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is CallSignalItem_Answer&&(identical(other.callId, callId) || other.callId == callId)&&(identical(other.sdp, sdp) || other.sdp == sdp));
}


@override
int get hashCode {
    return Object.hash(runtimeType,callId,sdp);
}

@override
String toString() {
    return 'CallSignalItem.answer(callId: $callId, sdp: $sdp)';
}


}

/// @nodoc
abstract mixin class $CallSignalItem_AnswerCopyWith<$Res> implements $CallSignalItemCopyWith<$Res> {
  factory $CallSignalItem_AnswerCopyWith(CallSignalItem_Answer value, $Res Function(CallSignalItem_Answer) _then) = _$CallSignalItem_AnswerCopyWithImpl;
@override @useResult
$Res call({
 String callId, String sdp
});




}
/// @nodoc
class _$CallSignalItem_AnswerCopyWithImpl<$Res>
    implements $CallSignalItem_AnswerCopyWith<$Res> {
  _$CallSignalItem_AnswerCopyWithImpl(this._self, this._then);

  final CallSignalItem_Answer _self;
  final $Res Function(CallSignalItem_Answer) _then;

/// Create a copy of CallSignalItem
/// with the given fields replaced by the non-null parameter values.
@override @pragma('vm:prefer-inline') $Res call({Object? callId = null,Object? sdp = null,}) {
  return _then(CallSignalItem_Answer(
callId: null == callId ? _self.callId : callId // ignore: cast_nullable_to_non_nullable
as String,sdp: null == sdp ? _self.sdp : sdp // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class CallSignalItem_Candidates extends CallSignalItem {
  const CallSignalItem_Candidates({required this.callId, required  List<String> candidates}): _candidates = candidates,super._();
  

/// Hex call id.
@override final  String callId;
/// Candidate lines.
 final  List<String> _candidates;
/// Candidate lines.
 List<String> get candidates {
  if (_candidates is EqualUnmodifiableListView) return _candidates;
  // ignore: implicit_dynamic_type
  return EqualUnmodifiableListView(_candidates);
}


/// Create a copy of CallSignalItem
/// with the given fields replaced by the non-null parameter values.
@override @JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$CallSignalItem_CandidatesCopyWith<CallSignalItem_Candidates> get copyWith => _$CallSignalItem_CandidatesCopyWithImpl<CallSignalItem_Candidates>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is CallSignalItem_Candidates&&(identical(other.callId, callId) || other.callId == callId)&&const DeepCollectionEquality().equals(other.candidates, _candidates));
}


@override
int get hashCode {
    return Object.hash(runtimeType,callId,const DeepCollectionEquality().hash(_candidates));
}

@override
String toString() {
    return 'CallSignalItem.candidates(callId: $callId, candidates: $candidates)';
}


}

/// @nodoc
abstract mixin class $CallSignalItem_CandidatesCopyWith<$Res> implements $CallSignalItemCopyWith<$Res> {
  factory $CallSignalItem_CandidatesCopyWith(CallSignalItem_Candidates value, $Res Function(CallSignalItem_Candidates) _then) = _$CallSignalItem_CandidatesCopyWithImpl;
@override @useResult
$Res call({
 String callId, List<String> candidates
});




}
/// @nodoc
class _$CallSignalItem_CandidatesCopyWithImpl<$Res>
    implements $CallSignalItem_CandidatesCopyWith<$Res> {
  _$CallSignalItem_CandidatesCopyWithImpl(this._self, this._then);

  final CallSignalItem_Candidates _self;
  final $Res Function(CallSignalItem_Candidates) _then;

/// Create a copy of CallSignalItem
/// with the given fields replaced by the non-null parameter values.
@override @pragma('vm:prefer-inline') $Res call({Object? callId = null,Object? candidates = null,}) {
  return _then(CallSignalItem_Candidates(
callId: null == callId ? _self.callId : callId // ignore: cast_nullable_to_non_nullable
as String,candidates: null == candidates ? _self._candidates : candidates // ignore: cast_nullable_to_non_nullable
as List<String>,
  ));
}


}

/// @nodoc


class CallSignalItem_End extends CallSignalItem {
  const CallSignalItem_End({required this.callId, required this.reason}): super._();
  

/// Hex call id.
@override final  String callId;
/// Why.
 final  CallEndReason reason;

/// Create a copy of CallSignalItem
/// with the given fields replaced by the non-null parameter values.
@override @JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$CallSignalItem_EndCopyWith<CallSignalItem_End> get copyWith => _$CallSignalItem_EndCopyWithImpl<CallSignalItem_End>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is CallSignalItem_End&&(identical(other.callId, callId) || other.callId == callId)&&(identical(other.reason, reason) || other.reason == reason));
}


@override
int get hashCode {
    return Object.hash(runtimeType,callId,reason);
}

@override
String toString() {
    return 'CallSignalItem.end(callId: $callId, reason: $reason)';
}


}

/// @nodoc
abstract mixin class $CallSignalItem_EndCopyWith<$Res> implements $CallSignalItemCopyWith<$Res> {
  factory $CallSignalItem_EndCopyWith(CallSignalItem_End value, $Res Function(CallSignalItem_End) _then) = _$CallSignalItem_EndCopyWithImpl;
@override @useResult
$Res call({
 String callId, CallEndReason reason
});




}
/// @nodoc
class _$CallSignalItem_EndCopyWithImpl<$Res>
    implements $CallSignalItem_EndCopyWith<$Res> {
  _$CallSignalItem_EndCopyWithImpl(this._self, this._then);

  final CallSignalItem_End _self;
  final $Res Function(CallSignalItem_End) _then;

/// Create a copy of CallSignalItem
/// with the given fields replaced by the non-null parameter values.
@override @pragma('vm:prefer-inline') $Res call({Object? callId = null,Object? reason = null,}) {
  return _then(CallSignalItem_End(
callId: null == callId ? _self.callId : callId // ignore: cast_nullable_to_non_nullable
as String,reason: null == reason ? _self.reason : reason // ignore: cast_nullable_to_non_nullable
as CallEndReason,
  ));
}


}

/// @nodoc
mixin _$NodeEventItem {





@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is NodeEventItem);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'NodeEventItem()';
}


}

/// @nodoc
class $NodeEventItemCopyWith<$Res>  {
$NodeEventItemCopyWith(NodeEventItem _, $Res Function(NodeEventItem) __);
}


/// Adds pattern-matching-related methods to [NodeEventItem].
extension NodeEventItemPatterns on NodeEventItem {
/// A variant of `map` that fallback to returning `orElse`.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case _:
///     return orElse();
/// }
/// ```

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( NodeEventItem_MessageReceived value)?  messageReceived,TResult Function( NodeEventItem_MessageStatus value)?  messageStatus,TResult Function( NodeEventItem_Reachability value)?  reachability,TResult Function( NodeEventItem_Contacts value)?  contacts,TResult Function( NodeEventItem_Requests value)?  requests,TResult Function( NodeEventItem_Groups value)?  groups,TResult Function( NodeEventItem_GroupMessage value)?  groupMessage,TResult Function( NodeEventItem_DeviceRetired value)?  deviceRetired,TResult Function( NodeEventItem_Call value)?  call,TResult Function( NodeEventItem_Shares value)?  shares,TResult Function( NodeEventItem_ShareWanted value)?  shareWanted,TResult Function( NodeEventItem_Traffic value)?  traffic,required TResult orElse(),}){
final _that = this;
switch (_that) {
case NodeEventItem_MessageReceived() when messageReceived != null:
return messageReceived(_that);case NodeEventItem_MessageStatus() when messageStatus != null:
return messageStatus(_that);case NodeEventItem_Reachability() when reachability != null:
return reachability(_that);case NodeEventItem_Contacts() when contacts != null:
return contacts(_that);case NodeEventItem_Requests() when requests != null:
return requests(_that);case NodeEventItem_Groups() when groups != null:
return groups(_that);case NodeEventItem_GroupMessage() when groupMessage != null:
return groupMessage(_that);case NodeEventItem_DeviceRetired() when deviceRetired != null:
return deviceRetired(_that);case NodeEventItem_Call() when call != null:
return call(_that);case NodeEventItem_Shares() when shares != null:
return shares(_that);case NodeEventItem_ShareWanted() when shareWanted != null:
return shareWanted(_that);case NodeEventItem_Traffic() when traffic != null:
return traffic(_that);case _:
  return orElse();

}
}
/// A `switch`-like method, using callbacks.
///
/// Callbacks receives the raw object, upcasted.
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case final Subclass2 value:
///     return ...;
/// }
/// ```

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( NodeEventItem_MessageReceived value)  messageReceived,required TResult Function( NodeEventItem_MessageStatus value)  messageStatus,required TResult Function( NodeEventItem_Reachability value)  reachability,required TResult Function( NodeEventItem_Contacts value)  contacts,required TResult Function( NodeEventItem_Requests value)  requests,required TResult Function( NodeEventItem_Groups value)  groups,required TResult Function( NodeEventItem_GroupMessage value)  groupMessage,required TResult Function( NodeEventItem_DeviceRetired value)  deviceRetired,required TResult Function( NodeEventItem_Call value)  call,required TResult Function( NodeEventItem_Shares value)  shares,required TResult Function( NodeEventItem_ShareWanted value)  shareWanted,required TResult Function( NodeEventItem_Traffic value)  traffic,}){
final _that = this;
switch (_that) {
case NodeEventItem_MessageReceived():
return messageReceived(_that);case NodeEventItem_MessageStatus():
return messageStatus(_that);case NodeEventItem_Reachability():
return reachability(_that);case NodeEventItem_Contacts():
return contacts(_that);case NodeEventItem_Requests():
return requests(_that);case NodeEventItem_Groups():
return groups(_that);case NodeEventItem_GroupMessage():
return groupMessage(_that);case NodeEventItem_DeviceRetired():
return deviceRetired(_that);case NodeEventItem_Call():
return call(_that);case NodeEventItem_Shares():
return shares(_that);case NodeEventItem_ShareWanted():
return shareWanted(_that);case NodeEventItem_Traffic():
return traffic(_that);}
}
/// A variant of `map` that fallback to returning `null`.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case _:
///     return null;
/// }
/// ```

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( NodeEventItem_MessageReceived value)?  messageReceived,TResult? Function( NodeEventItem_MessageStatus value)?  messageStatus,TResult? Function( NodeEventItem_Reachability value)?  reachability,TResult? Function( NodeEventItem_Contacts value)?  contacts,TResult? Function( NodeEventItem_Requests value)?  requests,TResult? Function( NodeEventItem_Groups value)?  groups,TResult? Function( NodeEventItem_GroupMessage value)?  groupMessage,TResult? Function( NodeEventItem_DeviceRetired value)?  deviceRetired,TResult? Function( NodeEventItem_Call value)?  call,TResult? Function( NodeEventItem_Shares value)?  shares,TResult? Function( NodeEventItem_ShareWanted value)?  shareWanted,TResult? Function( NodeEventItem_Traffic value)?  traffic,}){
final _that = this;
switch (_that) {
case NodeEventItem_MessageReceived() when messageReceived != null:
return messageReceived(_that);case NodeEventItem_MessageStatus() when messageStatus != null:
return messageStatus(_that);case NodeEventItem_Reachability() when reachability != null:
return reachability(_that);case NodeEventItem_Contacts() when contacts != null:
return contacts(_that);case NodeEventItem_Requests() when requests != null:
return requests(_that);case NodeEventItem_Groups() when groups != null:
return groups(_that);case NodeEventItem_GroupMessage() when groupMessage != null:
return groupMessage(_that);case NodeEventItem_DeviceRetired() when deviceRetired != null:
return deviceRetired(_that);case NodeEventItem_Call() when call != null:
return call(_that);case NodeEventItem_Shares() when shares != null:
return shares(_that);case NodeEventItem_ShareWanted() when shareWanted != null:
return shareWanted(_that);case NodeEventItem_Traffic() when traffic != null:
return traffic(_that);case _:
  return null;

}
}
/// A variant of `when` that fallback to an `orElse` callback.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case _:
///     return orElse();
/// }
/// ```

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function( String contactId)?  messageReceived,TResult Function( String messageId)?  messageStatus,TResult Function()?  reachability,TResult Function()?  contacts,TResult Function()?  requests,TResult Function()?  groups,TResult Function( String groupId)?  groupMessage,TResult Function()?  deviceRetired,TResult Function( String contactId)?  call,TResult Function()?  shares,TResult Function()?  shareWanted,TResult Function( String contactId,  Channel channel,  bool outgoing)?  traffic,required TResult orElse(),}) {final _that = this;
switch (_that) {
case NodeEventItem_MessageReceived() when messageReceived != null:
return messageReceived(_that.contactId);case NodeEventItem_MessageStatus() when messageStatus != null:
return messageStatus(_that.messageId);case NodeEventItem_Reachability() when reachability != null:
return reachability();case NodeEventItem_Contacts() when contacts != null:
return contacts();case NodeEventItem_Requests() when requests != null:
return requests();case NodeEventItem_Groups() when groups != null:
return groups();case NodeEventItem_GroupMessage() when groupMessage != null:
return groupMessage(_that.groupId);case NodeEventItem_DeviceRetired() when deviceRetired != null:
return deviceRetired();case NodeEventItem_Call() when call != null:
return call(_that.contactId);case NodeEventItem_Shares() when shares != null:
return shares();case NodeEventItem_ShareWanted() when shareWanted != null:
return shareWanted();case NodeEventItem_Traffic() when traffic != null:
return traffic(_that.contactId,_that.channel,_that.outgoing);case _:
  return orElse();

}
}
/// A `switch`-like method, using callbacks.
///
/// As opposed to `map`, this offers destructuring.
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case Subclass2(:final field2):
///     return ...;
/// }
/// ```

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function( String contactId)  messageReceived,required TResult Function( String messageId)  messageStatus,required TResult Function()  reachability,required TResult Function()  contacts,required TResult Function()  requests,required TResult Function()  groups,required TResult Function( String groupId)  groupMessage,required TResult Function()  deviceRetired,required TResult Function( String contactId)  call,required TResult Function()  shares,required TResult Function()  shareWanted,required TResult Function( String contactId,  Channel channel,  bool outgoing)  traffic,}) {final _that = this;
switch (_that) {
case NodeEventItem_MessageReceived():
return messageReceived(_that.contactId);case NodeEventItem_MessageStatus():
return messageStatus(_that.messageId);case NodeEventItem_Reachability():
return reachability();case NodeEventItem_Contacts():
return contacts();case NodeEventItem_Requests():
return requests();case NodeEventItem_Groups():
return groups();case NodeEventItem_GroupMessage():
return groupMessage(_that.groupId);case NodeEventItem_DeviceRetired():
return deviceRetired();case NodeEventItem_Call():
return call(_that.contactId);case NodeEventItem_Shares():
return shares();case NodeEventItem_ShareWanted():
return shareWanted();case NodeEventItem_Traffic():
return traffic(_that.contactId,_that.channel,_that.outgoing);}
}
/// A variant of `when` that fallback to returning `null`
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case _:
///     return null;
/// }
/// ```

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function( String contactId)?  messageReceived,TResult? Function( String messageId)?  messageStatus,TResult? Function()?  reachability,TResult? Function()?  contacts,TResult? Function()?  requests,TResult? Function()?  groups,TResult? Function( String groupId)?  groupMessage,TResult? Function()?  deviceRetired,TResult? Function( String contactId)?  call,TResult? Function()?  shares,TResult? Function()?  shareWanted,TResult? Function( String contactId,  Channel channel,  bool outgoing)?  traffic,}) {final _that = this;
switch (_that) {
case NodeEventItem_MessageReceived() when messageReceived != null:
return messageReceived(_that.contactId);case NodeEventItem_MessageStatus() when messageStatus != null:
return messageStatus(_that.messageId);case NodeEventItem_Reachability() when reachability != null:
return reachability();case NodeEventItem_Contacts() when contacts != null:
return contacts();case NodeEventItem_Requests() when requests != null:
return requests();case NodeEventItem_Groups() when groups != null:
return groups();case NodeEventItem_GroupMessage() when groupMessage != null:
return groupMessage(_that.groupId);case NodeEventItem_DeviceRetired() when deviceRetired != null:
return deviceRetired();case NodeEventItem_Call() when call != null:
return call(_that.contactId);case NodeEventItem_Shares() when shares != null:
return shares();case NodeEventItem_ShareWanted() when shareWanted != null:
return shareWanted();case NodeEventItem_Traffic() when traffic != null:
return traffic(_that.contactId,_that.channel,_that.outgoing);case _:
  return null;

}
}

}

/// @nodoc


class NodeEventItem_MessageReceived extends NodeEventItem {
  const NodeEventItem_MessageReceived({required this.contactId}): super._();
  

/// Contact hex id.
 final  String contactId;

/// Create a copy of NodeEventItem
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$NodeEventItem_MessageReceivedCopyWith<NodeEventItem_MessageReceived> get copyWith => _$NodeEventItem_MessageReceivedCopyWithImpl<NodeEventItem_MessageReceived>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is NodeEventItem_MessageReceived&&(identical(other.contactId, contactId) || other.contactId == contactId));
}


@override
int get hashCode {
    return Object.hash(runtimeType,contactId);
}

@override
String toString() {
    return 'NodeEventItem.messageReceived(contactId: $contactId)';
}


}

/// @nodoc
abstract mixin class $NodeEventItem_MessageReceivedCopyWith<$Res> implements $NodeEventItemCopyWith<$Res> {
  factory $NodeEventItem_MessageReceivedCopyWith(NodeEventItem_MessageReceived value, $Res Function(NodeEventItem_MessageReceived) _then) = _$NodeEventItem_MessageReceivedCopyWithImpl;
@useResult
$Res call({
 String contactId
});




}
/// @nodoc
class _$NodeEventItem_MessageReceivedCopyWithImpl<$Res>
    implements $NodeEventItem_MessageReceivedCopyWith<$Res> {
  _$NodeEventItem_MessageReceivedCopyWithImpl(this._self, this._then);

  final NodeEventItem_MessageReceived _self;
  final $Res Function(NodeEventItem_MessageReceived) _then;

/// Create a copy of NodeEventItem
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? contactId = null,}) {
  return _then(NodeEventItem_MessageReceived(
contactId: null == contactId ? _self.contactId : contactId // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class NodeEventItem_MessageStatus extends NodeEventItem {
  const NodeEventItem_MessageStatus({required this.messageId}): super._();
  

/// Message hex id.
 final  String messageId;

/// Create a copy of NodeEventItem
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$NodeEventItem_MessageStatusCopyWith<NodeEventItem_MessageStatus> get copyWith => _$NodeEventItem_MessageStatusCopyWithImpl<NodeEventItem_MessageStatus>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is NodeEventItem_MessageStatus&&(identical(other.messageId, messageId) || other.messageId == messageId));
}


@override
int get hashCode {
    return Object.hash(runtimeType,messageId);
}

@override
String toString() {
    return 'NodeEventItem.messageStatus(messageId: $messageId)';
}


}

/// @nodoc
abstract mixin class $NodeEventItem_MessageStatusCopyWith<$Res> implements $NodeEventItemCopyWith<$Res> {
  factory $NodeEventItem_MessageStatusCopyWith(NodeEventItem_MessageStatus value, $Res Function(NodeEventItem_MessageStatus) _then) = _$NodeEventItem_MessageStatusCopyWithImpl;
@useResult
$Res call({
 String messageId
});




}
/// @nodoc
class _$NodeEventItem_MessageStatusCopyWithImpl<$Res>
    implements $NodeEventItem_MessageStatusCopyWith<$Res> {
  _$NodeEventItem_MessageStatusCopyWithImpl(this._self, this._then);

  final NodeEventItem_MessageStatus _self;
  final $Res Function(NodeEventItem_MessageStatus) _then;

/// Create a copy of NodeEventItem
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? messageId = null,}) {
  return _then(NodeEventItem_MessageStatus(
messageId: null == messageId ? _self.messageId : messageId // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class NodeEventItem_Reachability extends NodeEventItem {
  const NodeEventItem_Reachability(): super._();
  






@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is NodeEventItem_Reachability);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'NodeEventItem.reachability()';
}


}




/// @nodoc


class NodeEventItem_Contacts extends NodeEventItem {
  const NodeEventItem_Contacts(): super._();
  






@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is NodeEventItem_Contacts);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'NodeEventItem.contacts()';
}


}




/// @nodoc


class NodeEventItem_Requests extends NodeEventItem {
  const NodeEventItem_Requests(): super._();
  






@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is NodeEventItem_Requests);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'NodeEventItem.requests()';
}


}




/// @nodoc


class NodeEventItem_Groups extends NodeEventItem {
  const NodeEventItem_Groups(): super._();
  






@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is NodeEventItem_Groups);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'NodeEventItem.groups()';
}


}




/// @nodoc


class NodeEventItem_GroupMessage extends NodeEventItem {
  const NodeEventItem_GroupMessage({required this.groupId}): super._();
  

/// Group hex id.
 final  String groupId;

/// Create a copy of NodeEventItem
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$NodeEventItem_GroupMessageCopyWith<NodeEventItem_GroupMessage> get copyWith => _$NodeEventItem_GroupMessageCopyWithImpl<NodeEventItem_GroupMessage>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is NodeEventItem_GroupMessage&&(identical(other.groupId, groupId) || other.groupId == groupId));
}


@override
int get hashCode {
    return Object.hash(runtimeType,groupId);
}

@override
String toString() {
    return 'NodeEventItem.groupMessage(groupId: $groupId)';
}


}

/// @nodoc
abstract mixin class $NodeEventItem_GroupMessageCopyWith<$Res> implements $NodeEventItemCopyWith<$Res> {
  factory $NodeEventItem_GroupMessageCopyWith(NodeEventItem_GroupMessage value, $Res Function(NodeEventItem_GroupMessage) _then) = _$NodeEventItem_GroupMessageCopyWithImpl;
@useResult
$Res call({
 String groupId
});




}
/// @nodoc
class _$NodeEventItem_GroupMessageCopyWithImpl<$Res>
    implements $NodeEventItem_GroupMessageCopyWith<$Res> {
  _$NodeEventItem_GroupMessageCopyWithImpl(this._self, this._then);

  final NodeEventItem_GroupMessage _self;
  final $Res Function(NodeEventItem_GroupMessage) _then;

/// Create a copy of NodeEventItem
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? groupId = null,}) {
  return _then(NodeEventItem_GroupMessage(
groupId: null == groupId ? _self.groupId : groupId // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class NodeEventItem_DeviceRetired extends NodeEventItem {
  const NodeEventItem_DeviceRetired(): super._();
  






@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is NodeEventItem_DeviceRetired);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'NodeEventItem.deviceRetired()';
}


}




/// @nodoc


class NodeEventItem_Call extends NodeEventItem {
  const NodeEventItem_Call({required this.contactId}): super._();
  

/// Contact hex id.
 final  String contactId;

/// Create a copy of NodeEventItem
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$NodeEventItem_CallCopyWith<NodeEventItem_Call> get copyWith => _$NodeEventItem_CallCopyWithImpl<NodeEventItem_Call>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is NodeEventItem_Call&&(identical(other.contactId, contactId) || other.contactId == contactId));
}


@override
int get hashCode {
    return Object.hash(runtimeType,contactId);
}

@override
String toString() {
    return 'NodeEventItem.call(contactId: $contactId)';
}


}

/// @nodoc
abstract mixin class $NodeEventItem_CallCopyWith<$Res> implements $NodeEventItemCopyWith<$Res> {
  factory $NodeEventItem_CallCopyWith(NodeEventItem_Call value, $Res Function(NodeEventItem_Call) _then) = _$NodeEventItem_CallCopyWithImpl;
@useResult
$Res call({
 String contactId
});




}
/// @nodoc
class _$NodeEventItem_CallCopyWithImpl<$Res>
    implements $NodeEventItem_CallCopyWith<$Res> {
  _$NodeEventItem_CallCopyWithImpl(this._self, this._then);

  final NodeEventItem_Call _self;
  final $Res Function(NodeEventItem_Call) _then;

/// Create a copy of NodeEventItem
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? contactId = null,}) {
  return _then(NodeEventItem_Call(
contactId: null == contactId ? _self.contactId : contactId // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class NodeEventItem_Shares extends NodeEventItem {
  const NodeEventItem_Shares(): super._();
  






@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is NodeEventItem_Shares);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'NodeEventItem.shares()';
}


}




/// @nodoc


class NodeEventItem_ShareWanted extends NodeEventItem {
  const NodeEventItem_ShareWanted(): super._();
  






@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is NodeEventItem_ShareWanted);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'NodeEventItem.shareWanted()';
}


}




/// @nodoc


class NodeEventItem_Traffic extends NodeEventItem {
  const NodeEventItem_Traffic({required this.contactId, required this.channel, required this.outgoing}): super._();
  

/// Contact hex id.
 final  String contactId;
/// The channel.
 final  Channel channel;
/// Sent by us.
 final  bool outgoing;

/// Create a copy of NodeEventItem
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$NodeEventItem_TrafficCopyWith<NodeEventItem_Traffic> get copyWith => _$NodeEventItem_TrafficCopyWithImpl<NodeEventItem_Traffic>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is NodeEventItem_Traffic&&(identical(other.contactId, contactId) || other.contactId == contactId)&&(identical(other.channel, channel) || other.channel == channel)&&(identical(other.outgoing, outgoing) || other.outgoing == outgoing));
}


@override
int get hashCode {
    return Object.hash(runtimeType,contactId,channel,outgoing);
}

@override
String toString() {
    return 'NodeEventItem.traffic(contactId: $contactId, channel: $channel, outgoing: $outgoing)';
}


}

/// @nodoc
abstract mixin class $NodeEventItem_TrafficCopyWith<$Res> implements $NodeEventItemCopyWith<$Res> {
  factory $NodeEventItem_TrafficCopyWith(NodeEventItem_Traffic value, $Res Function(NodeEventItem_Traffic) _then) = _$NodeEventItem_TrafficCopyWithImpl;
@useResult
$Res call({
 String contactId, Channel channel, bool outgoing
});




}
/// @nodoc
class _$NodeEventItem_TrafficCopyWithImpl<$Res>
    implements $NodeEventItem_TrafficCopyWith<$Res> {
  _$NodeEventItem_TrafficCopyWithImpl(this._self, this._then);

  final NodeEventItem_Traffic _self;
  final $Res Function(NodeEventItem_Traffic) _then;

/// Create a copy of NodeEventItem
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? contactId = null,Object? channel = null,Object? outgoing = null,}) {
  return _then(NodeEventItem_Traffic(
contactId: null == contactId ? _self.contactId : contactId // ignore: cast_nullable_to_non_nullable
as String,channel: null == channel ? _self.channel : channel // ignore: cast_nullable_to_non_nullable
as Channel,outgoing: null == outgoing ? _self.outgoing : outgoing // ignore: cast_nullable_to_non_nullable
as bool,
  ));
}


}

/// @nodoc
mixin _$ReceiveOutcome {





@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ReceiveOutcome);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'ReceiveOutcome()';
}


}

/// @nodoc
class $ReceiveOutcomeCopyWith<$Res>  {
$ReceiveOutcomeCopyWith(ReceiveOutcome _, $Res Function(ReceiveOutcome) __);
}


/// Adds pattern-matching-related methods to [ReceiveOutcome].
extension ReceiveOutcomePatterns on ReceiveOutcome {
/// A variant of `map` that fallback to returning `orElse`.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case _:
///     return orElse();
/// }
/// ```

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( ReceiveOutcome_Message value)?  message,TResult Function( ReceiveOutcome_Contact value)?  contact,TResult Function( ReceiveOutcome_Rejected value)?  rejected,TResult Function( ReceiveOutcome_Request value)?  request,TResult Function( ReceiveOutcome_NotMem value)?  notMem,required TResult orElse(),}){
final _that = this;
switch (_that) {
case ReceiveOutcome_Message() when message != null:
return message(_that);case ReceiveOutcome_Contact() when contact != null:
return contact(_that);case ReceiveOutcome_Rejected() when rejected != null:
return rejected(_that);case ReceiveOutcome_Request() when request != null:
return request(_that);case ReceiveOutcome_NotMem() when notMem != null:
return notMem(_that);case _:
  return orElse();

}
}
/// A `switch`-like method, using callbacks.
///
/// Callbacks receives the raw object, upcasted.
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case final Subclass2 value:
///     return ...;
/// }
/// ```

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( ReceiveOutcome_Message value)  message,required TResult Function( ReceiveOutcome_Contact value)  contact,required TResult Function( ReceiveOutcome_Rejected value)  rejected,required TResult Function( ReceiveOutcome_Request value)  request,required TResult Function( ReceiveOutcome_NotMem value)  notMem,}){
final _that = this;
switch (_that) {
case ReceiveOutcome_Message():
return message(_that);case ReceiveOutcome_Contact():
return contact(_that);case ReceiveOutcome_Rejected():
return rejected(_that);case ReceiveOutcome_Request():
return request(_that);case ReceiveOutcome_NotMem():
return notMem(_that);}
}
/// A variant of `map` that fallback to returning `null`.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case _:
///     return null;
/// }
/// ```

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( ReceiveOutcome_Message value)?  message,TResult? Function( ReceiveOutcome_Contact value)?  contact,TResult? Function( ReceiveOutcome_Rejected value)?  rejected,TResult? Function( ReceiveOutcome_Request value)?  request,TResult? Function( ReceiveOutcome_NotMem value)?  notMem,}){
final _that = this;
switch (_that) {
case ReceiveOutcome_Message() when message != null:
return message(_that);case ReceiveOutcome_Contact() when contact != null:
return contact(_that);case ReceiveOutcome_Rejected() when rejected != null:
return rejected(_that);case ReceiveOutcome_Request() when request != null:
return request(_that);case ReceiveOutcome_NotMem() when notMem != null:
return notMem(_that);case _:
  return null;

}
}
/// A variant of `when` that fallback to an `orElse` callback.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case _:
///     return orElse();
/// }
/// ```

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function( String contactId)?  message,TResult Function( String contactId,  bool updated,  bool unchanged,  String sas)?  contact,TResult Function( String reason)?  rejected,TResult Function()?  request,TResult Function()?  notMem,required TResult orElse(),}) {final _that = this;
switch (_that) {
case ReceiveOutcome_Message() when message != null:
return message(_that.contactId);case ReceiveOutcome_Contact() when contact != null:
return contact(_that.contactId,_that.updated,_that.unchanged,_that.sas);case ReceiveOutcome_Rejected() when rejected != null:
return rejected(_that.reason);case ReceiveOutcome_Request() when request != null:
return request();case ReceiveOutcome_NotMem() when notMem != null:
return notMem();case _:
  return orElse();

}
}
/// A `switch`-like method, using callbacks.
///
/// As opposed to `map`, this offers destructuring.
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case Subclass2(:final field2):
///     return ...;
/// }
/// ```

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function( String contactId)  message,required TResult Function( String contactId,  bool updated,  bool unchanged,  String sas)  contact,required TResult Function( String reason)  rejected,required TResult Function()  request,required TResult Function()  notMem,}) {final _that = this;
switch (_that) {
case ReceiveOutcome_Message():
return message(_that.contactId);case ReceiveOutcome_Contact():
return contact(_that.contactId,_that.updated,_that.unchanged,_that.sas);case ReceiveOutcome_Rejected():
return rejected(_that.reason);case ReceiveOutcome_Request():
return request();case ReceiveOutcome_NotMem():
return notMem();}
}
/// A variant of `when` that fallback to returning `null`
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case _:
///     return null;
/// }
/// ```

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function( String contactId)?  message,TResult? Function( String contactId,  bool updated,  bool unchanged,  String sas)?  contact,TResult? Function( String reason)?  rejected,TResult? Function()?  request,TResult? Function()?  notMem,}) {final _that = this;
switch (_that) {
case ReceiveOutcome_Message() when message != null:
return message(_that.contactId);case ReceiveOutcome_Contact() when contact != null:
return contact(_that.contactId,_that.updated,_that.unchanged,_that.sas);case ReceiveOutcome_Rejected() when rejected != null:
return rejected(_that.reason);case ReceiveOutcome_Request() when request != null:
return request();case ReceiveOutcome_NotMem() when notMem != null:
return notMem();case _:
  return null;

}
}

}

/// @nodoc


class ReceiveOutcome_Message extends ReceiveOutcome {
  const ReceiveOutcome_Message({required this.contactId}): super._();
  

/// Contact hex id.
 final  String contactId;

/// Create a copy of ReceiveOutcome
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ReceiveOutcome_MessageCopyWith<ReceiveOutcome_Message> get copyWith => _$ReceiveOutcome_MessageCopyWithImpl<ReceiveOutcome_Message>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ReceiveOutcome_Message&&(identical(other.contactId, contactId) || other.contactId == contactId));
}


@override
int get hashCode {
    return Object.hash(runtimeType,contactId);
}

@override
String toString() {
    return 'ReceiveOutcome.message(contactId: $contactId)';
}


}

/// @nodoc
abstract mixin class $ReceiveOutcome_MessageCopyWith<$Res> implements $ReceiveOutcomeCopyWith<$Res> {
  factory $ReceiveOutcome_MessageCopyWith(ReceiveOutcome_Message value, $Res Function(ReceiveOutcome_Message) _then) = _$ReceiveOutcome_MessageCopyWithImpl;
@useResult
$Res call({
 String contactId
});




}
/// @nodoc
class _$ReceiveOutcome_MessageCopyWithImpl<$Res>
    implements $ReceiveOutcome_MessageCopyWith<$Res> {
  _$ReceiveOutcome_MessageCopyWithImpl(this._self, this._then);

  final ReceiveOutcome_Message _self;
  final $Res Function(ReceiveOutcome_Message) _then;

/// Create a copy of ReceiveOutcome
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? contactId = null,}) {
  return _then(ReceiveOutcome_Message(
contactId: null == contactId ? _self.contactId : contactId // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class ReceiveOutcome_Contact extends ReceiveOutcome {
  const ReceiveOutcome_Contact({required this.contactId, required this.updated, required this.unchanged, required this.sas}): super._();
  

/// Contact hex id.
 final  String contactId;
/// Whether it already existed.
 final  bool updated;
/// Its card was read again and nothing needs checking: no symbols to compare.
 final  bool unchanged;
/// Short authentication string to compare.
 final  String sas;

/// Create a copy of ReceiveOutcome
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ReceiveOutcome_ContactCopyWith<ReceiveOutcome_Contact> get copyWith => _$ReceiveOutcome_ContactCopyWithImpl<ReceiveOutcome_Contact>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ReceiveOutcome_Contact&&(identical(other.contactId, contactId) || other.contactId == contactId)&&(identical(other.updated, updated) || other.updated == updated)&&(identical(other.unchanged, unchanged) || other.unchanged == unchanged)&&(identical(other.sas, sas) || other.sas == sas));
}


@override
int get hashCode {
    return Object.hash(runtimeType,contactId,updated,unchanged,sas);
}

@override
String toString() {
    return 'ReceiveOutcome.contact(contactId: $contactId, updated: $updated, unchanged: $unchanged, sas: $sas)';
}


}

/// @nodoc
abstract mixin class $ReceiveOutcome_ContactCopyWith<$Res> implements $ReceiveOutcomeCopyWith<$Res> {
  factory $ReceiveOutcome_ContactCopyWith(ReceiveOutcome_Contact value, $Res Function(ReceiveOutcome_Contact) _then) = _$ReceiveOutcome_ContactCopyWithImpl;
@useResult
$Res call({
 String contactId, bool updated, bool unchanged, String sas
});




}
/// @nodoc
class _$ReceiveOutcome_ContactCopyWithImpl<$Res>
    implements $ReceiveOutcome_ContactCopyWith<$Res> {
  _$ReceiveOutcome_ContactCopyWithImpl(this._self, this._then);

  final ReceiveOutcome_Contact _self;
  final $Res Function(ReceiveOutcome_Contact) _then;

/// Create a copy of ReceiveOutcome
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? contactId = null,Object? updated = null,Object? unchanged = null,Object? sas = null,}) {
  return _then(ReceiveOutcome_Contact(
contactId: null == contactId ? _self.contactId : contactId // ignore: cast_nullable_to_non_nullable
as String,updated: null == updated ? _self.updated : updated // ignore: cast_nullable_to_non_nullable
as bool,unchanged: null == unchanged ? _self.unchanged : unchanged // ignore: cast_nullable_to_non_nullable
as bool,sas: null == sas ? _self.sas : sas // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class ReceiveOutcome_Rejected extends ReceiveOutcome {
  const ReceiveOutcome_Rejected({required this.reason}): super._();
  

/// Reason key.
 final  String reason;

/// Create a copy of ReceiveOutcome
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ReceiveOutcome_RejectedCopyWith<ReceiveOutcome_Rejected> get copyWith => _$ReceiveOutcome_RejectedCopyWithImpl<ReceiveOutcome_Rejected>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ReceiveOutcome_Rejected&&(identical(other.reason, reason) || other.reason == reason));
}


@override
int get hashCode {
    return Object.hash(runtimeType,reason);
}

@override
String toString() {
    return 'ReceiveOutcome.rejected(reason: $reason)';
}


}

/// @nodoc
abstract mixin class $ReceiveOutcome_RejectedCopyWith<$Res> implements $ReceiveOutcomeCopyWith<$Res> {
  factory $ReceiveOutcome_RejectedCopyWith(ReceiveOutcome_Rejected value, $Res Function(ReceiveOutcome_Rejected) _then) = _$ReceiveOutcome_RejectedCopyWithImpl;
@useResult
$Res call({
 String reason
});




}
/// @nodoc
class _$ReceiveOutcome_RejectedCopyWithImpl<$Res>
    implements $ReceiveOutcome_RejectedCopyWith<$Res> {
  _$ReceiveOutcome_RejectedCopyWithImpl(this._self, this._then);

  final ReceiveOutcome_Rejected _self;
  final $Res Function(ReceiveOutcome_Rejected) _then;

/// Create a copy of ReceiveOutcome
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? reason = null,}) {
  return _then(ReceiveOutcome_Rejected(
reason: null == reason ? _self.reason : reason // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class ReceiveOutcome_Request extends ReceiveOutcome {
  const ReceiveOutcome_Request(): super._();
  






@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ReceiveOutcome_Request);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'ReceiveOutcome.request()';
}


}




/// @nodoc


class ReceiveOutcome_NotMem extends ReceiveOutcome {
  const ReceiveOutcome_NotMem(): super._();
  






@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ReceiveOutcome_NotMem);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'ReceiveOutcome.notMem()';
}


}




/// @nodoc
mixin _$ScanUpdate {





@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ScanUpdate);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'ScanUpdate()';
}


}

/// @nodoc
class $ScanUpdateCopyWith<$Res>  {
$ScanUpdateCopyWith(ScanUpdate _, $Res Function(ScanUpdate) __);
}


/// Adds pattern-matching-related methods to [ScanUpdate].
extension ScanUpdatePatterns on ScanUpdate {
/// A variant of `map` that fallback to returning `orElse`.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case _:
///     return orElse();
/// }
/// ```

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( ScanUpdate_Nothing value)?  nothing,TResult Function( ScanUpdate_Progress value)?  progress,TResult Function( ScanUpdate_Received value)?  received,TResult Function( ScanUpdate_Backup value)?  backup,TResult Function( ScanUpdate_DeviceSetup value)?  deviceSetup,TResult Function( ScanUpdate_DeviceRequest value)?  deviceRequest,TResult Function( ScanUpdate_Connecting value)?  connecting,TResult Function( ScanUpdate_Attest value)?  attest,required TResult orElse(),}){
final _that = this;
switch (_that) {
case ScanUpdate_Nothing() when nothing != null:
return nothing(_that);case ScanUpdate_Progress() when progress != null:
return progress(_that);case ScanUpdate_Received() when received != null:
return received(_that);case ScanUpdate_Backup() when backup != null:
return backup(_that);case ScanUpdate_DeviceSetup() when deviceSetup != null:
return deviceSetup(_that);case ScanUpdate_DeviceRequest() when deviceRequest != null:
return deviceRequest(_that);case ScanUpdate_Connecting() when connecting != null:
return connecting(_that);case ScanUpdate_Attest() when attest != null:
return attest(_that);case _:
  return orElse();

}
}
/// A `switch`-like method, using callbacks.
///
/// Callbacks receives the raw object, upcasted.
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case final Subclass2 value:
///     return ...;
/// }
/// ```

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( ScanUpdate_Nothing value)  nothing,required TResult Function( ScanUpdate_Progress value)  progress,required TResult Function( ScanUpdate_Received value)  received,required TResult Function( ScanUpdate_Backup value)  backup,required TResult Function( ScanUpdate_DeviceSetup value)  deviceSetup,required TResult Function( ScanUpdate_DeviceRequest value)  deviceRequest,required TResult Function( ScanUpdate_Connecting value)  connecting,required TResult Function( ScanUpdate_Attest value)  attest,}){
final _that = this;
switch (_that) {
case ScanUpdate_Nothing():
return nothing(_that);case ScanUpdate_Progress():
return progress(_that);case ScanUpdate_Received():
return received(_that);case ScanUpdate_Backup():
return backup(_that);case ScanUpdate_DeviceSetup():
return deviceSetup(_that);case ScanUpdate_DeviceRequest():
return deviceRequest(_that);case ScanUpdate_Connecting():
return connecting(_that);case ScanUpdate_Attest():
return attest(_that);}
}
/// A variant of `map` that fallback to returning `null`.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case _:
///     return null;
/// }
/// ```

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( ScanUpdate_Nothing value)?  nothing,TResult? Function( ScanUpdate_Progress value)?  progress,TResult? Function( ScanUpdate_Received value)?  received,TResult? Function( ScanUpdate_Backup value)?  backup,TResult? Function( ScanUpdate_DeviceSetup value)?  deviceSetup,TResult? Function( ScanUpdate_DeviceRequest value)?  deviceRequest,TResult? Function( ScanUpdate_Connecting value)?  connecting,TResult? Function( ScanUpdate_Attest value)?  attest,}){
final _that = this;
switch (_that) {
case ScanUpdate_Nothing() when nothing != null:
return nothing(_that);case ScanUpdate_Progress() when progress != null:
return progress(_that);case ScanUpdate_Received() when received != null:
return received(_that);case ScanUpdate_Backup() when backup != null:
return backup(_that);case ScanUpdate_DeviceSetup() when deviceSetup != null:
return deviceSetup(_that);case ScanUpdate_DeviceRequest() when deviceRequest != null:
return deviceRequest(_that);case ScanUpdate_Connecting() when connecting != null:
return connecting(_that);case ScanUpdate_Attest() when attest != null:
return attest(_that);case _:
  return null;

}
}
/// A variant of `when` that fallback to an `orElse` callback.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case _:
///     return orElse();
/// }
/// ```

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function()?  nothing,TResult Function( int received,  int needed)?  progress,TResult Function( ReceiveOutcome field0)?  received,TResult Function( Uint8List field0)?  backup,TResult Function( Uint8List field0)?  deviceSetup,TResult Function( Uint8List field0)?  deviceRequest,TResult Function()?  connecting,TResult Function( String field0)?  attest,required TResult orElse(),}) {final _that = this;
switch (_that) {
case ScanUpdate_Nothing() when nothing != null:
return nothing();case ScanUpdate_Progress() when progress != null:
return progress(_that.received,_that.needed);case ScanUpdate_Received() when received != null:
return received(_that.field0);case ScanUpdate_Backup() when backup != null:
return backup(_that.field0);case ScanUpdate_DeviceSetup() when deviceSetup != null:
return deviceSetup(_that.field0);case ScanUpdate_DeviceRequest() when deviceRequest != null:
return deviceRequest(_that.field0);case ScanUpdate_Connecting() when connecting != null:
return connecting();case ScanUpdate_Attest() when attest != null:
return attest(_that.field0);case _:
  return orElse();

}
}
/// A `switch`-like method, using callbacks.
///
/// As opposed to `map`, this offers destructuring.
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case Subclass2(:final field2):
///     return ...;
/// }
/// ```

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function()  nothing,required TResult Function( int received,  int needed)  progress,required TResult Function( ReceiveOutcome field0)  received,required TResult Function( Uint8List field0)  backup,required TResult Function( Uint8List field0)  deviceSetup,required TResult Function( Uint8List field0)  deviceRequest,required TResult Function()  connecting,required TResult Function( String field0)  attest,}) {final _that = this;
switch (_that) {
case ScanUpdate_Nothing():
return nothing();case ScanUpdate_Progress():
return progress(_that.received,_that.needed);case ScanUpdate_Received():
return received(_that.field0);case ScanUpdate_Backup():
return backup(_that.field0);case ScanUpdate_DeviceSetup():
return deviceSetup(_that.field0);case ScanUpdate_DeviceRequest():
return deviceRequest(_that.field0);case ScanUpdate_Connecting():
return connecting();case ScanUpdate_Attest():
return attest(_that.field0);}
}
/// A variant of `when` that fallback to returning `null`
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case _:
///     return null;
/// }
/// ```

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function()?  nothing,TResult? Function( int received,  int needed)?  progress,TResult? Function( ReceiveOutcome field0)?  received,TResult? Function( Uint8List field0)?  backup,TResult? Function( Uint8List field0)?  deviceSetup,TResult? Function( Uint8List field0)?  deviceRequest,TResult? Function()?  connecting,TResult? Function( String field0)?  attest,}) {final _that = this;
switch (_that) {
case ScanUpdate_Nothing() when nothing != null:
return nothing();case ScanUpdate_Progress() when progress != null:
return progress(_that.received,_that.needed);case ScanUpdate_Received() when received != null:
return received(_that.field0);case ScanUpdate_Backup() when backup != null:
return backup(_that.field0);case ScanUpdate_DeviceSetup() when deviceSetup != null:
return deviceSetup(_that.field0);case ScanUpdate_DeviceRequest() when deviceRequest != null:
return deviceRequest(_that.field0);case ScanUpdate_Connecting() when connecting != null:
return connecting();case ScanUpdate_Attest() when attest != null:
return attest(_that.field0);case _:
  return null;

}
}

}

/// @nodoc


class ScanUpdate_Nothing extends ScanUpdate {
  const ScanUpdate_Nothing(): super._();
  






@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ScanUpdate_Nothing);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'ScanUpdate.nothing()';
}


}




/// @nodoc


class ScanUpdate_Progress extends ScanUpdate {
  const ScanUpdate_Progress({required this.received, required this.needed}): super._();
  

/// Frames collected.
 final  int received;
/// Frames needed.
 final  int needed;

/// Create a copy of ScanUpdate
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ScanUpdate_ProgressCopyWith<ScanUpdate_Progress> get copyWith => _$ScanUpdate_ProgressCopyWithImpl<ScanUpdate_Progress>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ScanUpdate_Progress&&(identical(other.received, received) || other.received == received)&&(identical(other.needed, needed) || other.needed == needed));
}


@override
int get hashCode {
    return Object.hash(runtimeType,received,needed);
}

@override
String toString() {
    return 'ScanUpdate.progress(received: $received, needed: $needed)';
}


}

/// @nodoc
abstract mixin class $ScanUpdate_ProgressCopyWith<$Res> implements $ScanUpdateCopyWith<$Res> {
  factory $ScanUpdate_ProgressCopyWith(ScanUpdate_Progress value, $Res Function(ScanUpdate_Progress) _then) = _$ScanUpdate_ProgressCopyWithImpl;
@useResult
$Res call({
 int received, int needed
});




}
/// @nodoc
class _$ScanUpdate_ProgressCopyWithImpl<$Res>
    implements $ScanUpdate_ProgressCopyWith<$Res> {
  _$ScanUpdate_ProgressCopyWithImpl(this._self, this._then);

  final ScanUpdate_Progress _self;
  final $Res Function(ScanUpdate_Progress) _then;

/// Create a copy of ScanUpdate
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? received = null,Object? needed = null,}) {
  return _then(ScanUpdate_Progress(
received: null == received ? _self.received : received // ignore: cast_nullable_to_non_nullable
as int,needed: null == needed ? _self.needed : needed // ignore: cast_nullable_to_non_nullable
as int,
  ));
}


}

/// @nodoc


class ScanUpdate_Received extends ScanUpdate {
  const ScanUpdate_Received(this.field0): super._();
  

 final  ReceiveOutcome field0;

/// Create a copy of ScanUpdate
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ScanUpdate_ReceivedCopyWith<ScanUpdate_Received> get copyWith => _$ScanUpdate_ReceivedCopyWithImpl<ScanUpdate_Received>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ScanUpdate_Received&&(identical(other.field0, field0) || other.field0 == field0));
}


@override
int get hashCode {
    return Object.hash(runtimeType,field0);
}

@override
String toString() {
    return 'ScanUpdate.received(field0: $field0)';
}


}

/// @nodoc
abstract mixin class $ScanUpdate_ReceivedCopyWith<$Res> implements $ScanUpdateCopyWith<$Res> {
  factory $ScanUpdate_ReceivedCopyWith(ScanUpdate_Received value, $Res Function(ScanUpdate_Received) _then) = _$ScanUpdate_ReceivedCopyWithImpl;
@useResult
$Res call({
 ReceiveOutcome field0
});


$ReceiveOutcomeCopyWith<$Res> get field0;

}
/// @nodoc
class _$ScanUpdate_ReceivedCopyWithImpl<$Res>
    implements $ScanUpdate_ReceivedCopyWith<$Res> {
  _$ScanUpdate_ReceivedCopyWithImpl(this._self, this._then);

  final ScanUpdate_Received _self;
  final $Res Function(ScanUpdate_Received) _then;

/// Create a copy of ScanUpdate
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? field0 = null,}) {
  return _then(ScanUpdate_Received(
null == field0 ? _self.field0 : field0 // ignore: cast_nullable_to_non_nullable
as ReceiveOutcome,
  ));
}

/// Create a copy of ScanUpdate
/// with the given fields replaced by the non-null parameter values.
@override
@pragma('vm:prefer-inline')
$ReceiveOutcomeCopyWith<$Res> get field0 {
  
  return $ReceiveOutcomeCopyWith<$Res>(_self.field0, (value) {
    return _then(_self.copyWith(field0: value));
  });
}
}

/// @nodoc


class ScanUpdate_Backup extends ScanUpdate {
  const ScanUpdate_Backup(this.field0): super._();
  

 final  Uint8List field0;

/// Create a copy of ScanUpdate
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ScanUpdate_BackupCopyWith<ScanUpdate_Backup> get copyWith => _$ScanUpdate_BackupCopyWithImpl<ScanUpdate_Backup>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ScanUpdate_Backup&&const DeepCollectionEquality().equals(other.field0, field0));
}


@override
int get hashCode {
    return Object.hash(runtimeType,const DeepCollectionEquality().hash(field0));
}

@override
String toString() {
    return 'ScanUpdate.backup(field0: $field0)';
}


}

/// @nodoc
abstract mixin class $ScanUpdate_BackupCopyWith<$Res> implements $ScanUpdateCopyWith<$Res> {
  factory $ScanUpdate_BackupCopyWith(ScanUpdate_Backup value, $Res Function(ScanUpdate_Backup) _then) = _$ScanUpdate_BackupCopyWithImpl;
@useResult
$Res call({
 Uint8List field0
});




}
/// @nodoc
class _$ScanUpdate_BackupCopyWithImpl<$Res>
    implements $ScanUpdate_BackupCopyWith<$Res> {
  _$ScanUpdate_BackupCopyWithImpl(this._self, this._then);

  final ScanUpdate_Backup _self;
  final $Res Function(ScanUpdate_Backup) _then;

/// Create a copy of ScanUpdate
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? field0 = null,}) {
  return _then(ScanUpdate_Backup(
null == field0 ? _self.field0 : field0 // ignore: cast_nullable_to_non_nullable
as Uint8List,
  ));
}


}

/// @nodoc


class ScanUpdate_DeviceSetup extends ScanUpdate {
  const ScanUpdate_DeviceSetup(this.field0): super._();
  

 final  Uint8List field0;

/// Create a copy of ScanUpdate
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ScanUpdate_DeviceSetupCopyWith<ScanUpdate_DeviceSetup> get copyWith => _$ScanUpdate_DeviceSetupCopyWithImpl<ScanUpdate_DeviceSetup>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ScanUpdate_DeviceSetup&&const DeepCollectionEquality().equals(other.field0, field0));
}


@override
int get hashCode {
    return Object.hash(runtimeType,const DeepCollectionEquality().hash(field0));
}

@override
String toString() {
    return 'ScanUpdate.deviceSetup(field0: $field0)';
}


}

/// @nodoc
abstract mixin class $ScanUpdate_DeviceSetupCopyWith<$Res> implements $ScanUpdateCopyWith<$Res> {
  factory $ScanUpdate_DeviceSetupCopyWith(ScanUpdate_DeviceSetup value, $Res Function(ScanUpdate_DeviceSetup) _then) = _$ScanUpdate_DeviceSetupCopyWithImpl;
@useResult
$Res call({
 Uint8List field0
});




}
/// @nodoc
class _$ScanUpdate_DeviceSetupCopyWithImpl<$Res>
    implements $ScanUpdate_DeviceSetupCopyWith<$Res> {
  _$ScanUpdate_DeviceSetupCopyWithImpl(this._self, this._then);

  final ScanUpdate_DeviceSetup _self;
  final $Res Function(ScanUpdate_DeviceSetup) _then;

/// Create a copy of ScanUpdate
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? field0 = null,}) {
  return _then(ScanUpdate_DeviceSetup(
null == field0 ? _self.field0 : field0 // ignore: cast_nullable_to_non_nullable
as Uint8List,
  ));
}


}

/// @nodoc


class ScanUpdate_DeviceRequest extends ScanUpdate {
  const ScanUpdate_DeviceRequest(this.field0): super._();
  

 final  Uint8List field0;

/// Create a copy of ScanUpdate
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ScanUpdate_DeviceRequestCopyWith<ScanUpdate_DeviceRequest> get copyWith => _$ScanUpdate_DeviceRequestCopyWithImpl<ScanUpdate_DeviceRequest>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ScanUpdate_DeviceRequest&&const DeepCollectionEquality().equals(other.field0, field0));
}


@override
int get hashCode {
    return Object.hash(runtimeType,const DeepCollectionEquality().hash(field0));
}

@override
String toString() {
    return 'ScanUpdate.deviceRequest(field0: $field0)';
}


}

/// @nodoc
abstract mixin class $ScanUpdate_DeviceRequestCopyWith<$Res> implements $ScanUpdateCopyWith<$Res> {
  factory $ScanUpdate_DeviceRequestCopyWith(ScanUpdate_DeviceRequest value, $Res Function(ScanUpdate_DeviceRequest) _then) = _$ScanUpdate_DeviceRequestCopyWithImpl;
@useResult
$Res call({
 Uint8List field0
});




}
/// @nodoc
class _$ScanUpdate_DeviceRequestCopyWithImpl<$Res>
    implements $ScanUpdate_DeviceRequestCopyWith<$Res> {
  _$ScanUpdate_DeviceRequestCopyWithImpl(this._self, this._then);

  final ScanUpdate_DeviceRequest _self;
  final $Res Function(ScanUpdate_DeviceRequest) _then;

/// Create a copy of ScanUpdate
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? field0 = null,}) {
  return _then(ScanUpdate_DeviceRequest(
null == field0 ? _self.field0 : field0 // ignore: cast_nullable_to_non_nullable
as Uint8List,
  ));
}


}

/// @nodoc


class ScanUpdate_Connecting extends ScanUpdate {
  const ScanUpdate_Connecting(): super._();
  






@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ScanUpdate_Connecting);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'ScanUpdate.connecting()';
}


}




/// @nodoc


class ScanUpdate_Attest extends ScanUpdate {
  const ScanUpdate_Attest(this.field0): super._();
  

 final  String field0;

/// Create a copy of ScanUpdate
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$ScanUpdate_AttestCopyWith<ScanUpdate_Attest> get copyWith => _$ScanUpdate_AttestCopyWithImpl<ScanUpdate_Attest>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is ScanUpdate_Attest&&(identical(other.field0, field0) || other.field0 == field0));
}


@override
int get hashCode {
    return Object.hash(runtimeType,field0);
}

@override
String toString() {
    return 'ScanUpdate.attest(field0: $field0)';
}


}

/// @nodoc
abstract mixin class $ScanUpdate_AttestCopyWith<$Res> implements $ScanUpdateCopyWith<$Res> {
  factory $ScanUpdate_AttestCopyWith(ScanUpdate_Attest value, $Res Function(ScanUpdate_Attest) _then) = _$ScanUpdate_AttestCopyWithImpl;
@useResult
$Res call({
 String field0
});




}
/// @nodoc
class _$ScanUpdate_AttestCopyWithImpl<$Res>
    implements $ScanUpdate_AttestCopyWith<$Res> {
  _$ScanUpdate_AttestCopyWithImpl(this._self, this._then);

  final ScanUpdate_Attest _self;
  final $Res Function(ScanUpdate_Attest) _then;

/// Create a copy of ScanUpdate
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? field0 = null,}) {
  return _then(ScanUpdate_Attest(
null == field0 ? _self.field0 : field0 // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc
mixin _$SharedFileState {





@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is SharedFileState);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'SharedFileState()';
}


}

/// @nodoc
class $SharedFileStateCopyWith<$Res>  {
$SharedFileStateCopyWith(SharedFileState _, $Res Function(SharedFileState) __);
}


/// Adds pattern-matching-related methods to [SharedFileState].
extension SharedFileStatePatterns on SharedFileState {
/// A variant of `map` that fallback to returning `orElse`.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case _:
///     return orElse();
/// }
/// ```

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( SharedFileState_Available value)?  available,TResult Function( SharedFileState_Requested value)?  requested,TResult Function( SharedFileState_Received value)?  received,required TResult orElse(),}){
final _that = this;
switch (_that) {
case SharedFileState_Available() when available != null:
return available(_that);case SharedFileState_Requested() when requested != null:
return requested(_that);case SharedFileState_Received() when received != null:
return received(_that);case _:
  return orElse();

}
}
/// A `switch`-like method, using callbacks.
///
/// Callbacks receives the raw object, upcasted.
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case final Subclass2 value:
///     return ...;
/// }
/// ```

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( SharedFileState_Available value)  available,required TResult Function( SharedFileState_Requested value)  requested,required TResult Function( SharedFileState_Received value)  received,}){
final _that = this;
switch (_that) {
case SharedFileState_Available():
return available(_that);case SharedFileState_Requested():
return requested(_that);case SharedFileState_Received():
return received(_that);}
}
/// A variant of `map` that fallback to returning `null`.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case _:
///     return null;
/// }
/// ```

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( SharedFileState_Available value)?  available,TResult? Function( SharedFileState_Requested value)?  requested,TResult? Function( SharedFileState_Received value)?  received,}){
final _that = this;
switch (_that) {
case SharedFileState_Available() when available != null:
return available(_that);case SharedFileState_Requested() when requested != null:
return requested(_that);case SharedFileState_Received() when received != null:
return received(_that);case _:
  return null;

}
}
/// A variant of `when` that fallback to an `orElse` callback.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case _:
///     return orElse();
/// }
/// ```

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function()?  available,TResult Function()?  requested,TResult Function( String messageId)?  received,required TResult orElse(),}) {final _that = this;
switch (_that) {
case SharedFileState_Available() when available != null:
return available();case SharedFileState_Requested() when requested != null:
return requested();case SharedFileState_Received() when received != null:
return received(_that.messageId);case _:
  return orElse();

}
}
/// A `switch`-like method, using callbacks.
///
/// As opposed to `map`, this offers destructuring.
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case Subclass2(:final field2):
///     return ...;
/// }
/// ```

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function()  available,required TResult Function()  requested,required TResult Function( String messageId)  received,}) {final _that = this;
switch (_that) {
case SharedFileState_Available():
return available();case SharedFileState_Requested():
return requested();case SharedFileState_Received():
return received(_that.messageId);}
}
/// A variant of `when` that fallback to returning `null`
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case _:
///     return null;
/// }
/// ```

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function()?  available,TResult? Function()?  requested,TResult? Function( String messageId)?  received,}) {final _that = this;
switch (_that) {
case SharedFileState_Available() when available != null:
return available();case SharedFileState_Requested() when requested != null:
return requested();case SharedFileState_Received() when received != null:
return received(_that.messageId);case _:
  return null;

}
}

}

/// @nodoc


class SharedFileState_Available extends SharedFileState {
  const SharedFileState_Available(): super._();
  






@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is SharedFileState_Available);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'SharedFileState.available()';
}


}




/// @nodoc


class SharedFileState_Requested extends SharedFileState {
  const SharedFileState_Requested(): super._();
  






@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is SharedFileState_Requested);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'SharedFileState.requested()';
}


}




/// @nodoc


class SharedFileState_Received extends SharedFileState {
  const SharedFileState_Received({required this.messageId}): super._();
  

/// Message hex id.
 final  String messageId;

/// Create a copy of SharedFileState
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$SharedFileState_ReceivedCopyWith<SharedFileState_Received> get copyWith => _$SharedFileState_ReceivedCopyWithImpl<SharedFileState_Received>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is SharedFileState_Received&&(identical(other.messageId, messageId) || other.messageId == messageId));
}


@override
int get hashCode {
    return Object.hash(runtimeType,messageId);
}

@override
String toString() {
    return 'SharedFileState.received(messageId: $messageId)';
}


}

/// @nodoc
abstract mixin class $SharedFileState_ReceivedCopyWith<$Res> implements $SharedFileStateCopyWith<$Res> {
  factory $SharedFileState_ReceivedCopyWith(SharedFileState_Received value, $Res Function(SharedFileState_Received) _then) = _$SharedFileState_ReceivedCopyWithImpl;
@useResult
$Res call({
 String messageId
});




}
/// @nodoc
class _$SharedFileState_ReceivedCopyWithImpl<$Res>
    implements $SharedFileState_ReceivedCopyWith<$Res> {
  _$SharedFileState_ReceivedCopyWithImpl(this._self, this._then);

  final SharedFileState_Received _self;
  final $Res Function(SharedFileState_Received) _then;

/// Create a copy of SharedFileState
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? messageId = null,}) {
  return _then(SharedFileState_Received(
messageId: null == messageId ? _self.messageId : messageId // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc
mixin _$TransmitOutcome {





@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is TransmitOutcome);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'TransmitOutcome()';
}


}

/// @nodoc
class $TransmitOutcomeCopyWith<$Res>  {
$TransmitOutcomeCopyWith(TransmitOutcome _, $Res Function(TransmitOutcome) __);
}


/// Adds pattern-matching-related methods to [TransmitOutcome].
extension TransmitOutcomePatterns on TransmitOutcome {
/// A variant of `map` that fallback to returning `orElse`.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case _:
///     return orElse();
/// }
/// ```

@optionalTypeArgs TResult maybeMap<TResult extends Object?>({TResult Function( TransmitOutcome_Ready value)?  ready,TResult Function( TransmitOutcome_Sending value)?  sending,TResult Function( TransmitOutcome_CourierQueued value)?  courierQueued,TResult Function( TransmitOutcome_PlainReady value)?  plainReady,TResult Function( TransmitOutcome_ConsentRequired value)?  consentRequired,TResult Function( TransmitOutcome_Rejected value)?  rejected,required TResult orElse(),}){
final _that = this;
switch (_that) {
case TransmitOutcome_Ready() when ready != null:
return ready(_that);case TransmitOutcome_Sending() when sending != null:
return sending(_that);case TransmitOutcome_CourierQueued() when courierQueued != null:
return courierQueued(_that);case TransmitOutcome_PlainReady() when plainReady != null:
return plainReady(_that);case TransmitOutcome_ConsentRequired() when consentRequired != null:
return consentRequired(_that);case TransmitOutcome_Rejected() when rejected != null:
return rejected(_that);case _:
  return orElse();

}
}
/// A `switch`-like method, using callbacks.
///
/// Callbacks receives the raw object, upcasted.
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case final Subclass2 value:
///     return ...;
/// }
/// ```

@optionalTypeArgs TResult map<TResult extends Object?>({required TResult Function( TransmitOutcome_Ready value)  ready,required TResult Function( TransmitOutcome_Sending value)  sending,required TResult Function( TransmitOutcome_CourierQueued value)  courierQueued,required TResult Function( TransmitOutcome_PlainReady value)  plainReady,required TResult Function( TransmitOutcome_ConsentRequired value)  consentRequired,required TResult Function( TransmitOutcome_Rejected value)  rejected,}){
final _that = this;
switch (_that) {
case TransmitOutcome_Ready():
return ready(_that);case TransmitOutcome_Sending():
return sending(_that);case TransmitOutcome_CourierQueued():
return courierQueued(_that);case TransmitOutcome_PlainReady():
return plainReady(_that);case TransmitOutcome_ConsentRequired():
return consentRequired(_that);case TransmitOutcome_Rejected():
return rejected(_that);}
}
/// A variant of `map` that fallback to returning `null`.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case final Subclass value:
///     return ...;
///   case _:
///     return null;
/// }
/// ```

@optionalTypeArgs TResult? mapOrNull<TResult extends Object?>({TResult? Function( TransmitOutcome_Ready value)?  ready,TResult? Function( TransmitOutcome_Sending value)?  sending,TResult? Function( TransmitOutcome_CourierQueued value)?  courierQueued,TResult? Function( TransmitOutcome_PlainReady value)?  plainReady,TResult? Function( TransmitOutcome_ConsentRequired value)?  consentRequired,TResult? Function( TransmitOutcome_Rejected value)?  rejected,}){
final _that = this;
switch (_that) {
case TransmitOutcome_Ready() when ready != null:
return ready(_that);case TransmitOutcome_Sending() when sending != null:
return sending(_that);case TransmitOutcome_CourierQueued() when courierQueued != null:
return courierQueued(_that);case TransmitOutcome_PlainReady() when plainReady != null:
return plainReady(_that);case TransmitOutcome_ConsentRequired() when consentRequired != null:
return consentRequired(_that);case TransmitOutcome_Rejected() when rejected != null:
return rejected(_that);case _:
  return null;

}
}
/// A variant of `when` that fallback to an `orElse` callback.
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case _:
///     return orElse();
/// }
/// ```

@optionalTypeArgs TResult maybeWhen<TResult extends Object?>({TResult Function( BigInt op,  Channel channel,  String text,  QrAnimation? animation)?  ready,TResult Function( Channel channel)?  sending,TResult Function()?  courierQueued,TResult Function( BigInt op,  String text)?  plainReady,TResult Function( ConsentRequest field0)?  consentRequired,TResult Function( String reason)?  rejected,required TResult orElse(),}) {final _that = this;
switch (_that) {
case TransmitOutcome_Ready() when ready != null:
return ready(_that.op,_that.channel,_that.text,_that.animation);case TransmitOutcome_Sending() when sending != null:
return sending(_that.channel);case TransmitOutcome_CourierQueued() when courierQueued != null:
return courierQueued();case TransmitOutcome_PlainReady() when plainReady != null:
return plainReady(_that.op,_that.text);case TransmitOutcome_ConsentRequired() when consentRequired != null:
return consentRequired(_that.field0);case TransmitOutcome_Rejected() when rejected != null:
return rejected(_that.reason);case _:
  return orElse();

}
}
/// A `switch`-like method, using callbacks.
///
/// As opposed to `map`, this offers destructuring.
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case Subclass2(:final field2):
///     return ...;
/// }
/// ```

@optionalTypeArgs TResult when<TResult extends Object?>({required TResult Function( BigInt op,  Channel channel,  String text,  QrAnimation? animation)  ready,required TResult Function( Channel channel)  sending,required TResult Function()  courierQueued,required TResult Function( BigInt op,  String text)  plainReady,required TResult Function( ConsentRequest field0)  consentRequired,required TResult Function( String reason)  rejected,}) {final _that = this;
switch (_that) {
case TransmitOutcome_Ready():
return ready(_that.op,_that.channel,_that.text,_that.animation);case TransmitOutcome_Sending():
return sending(_that.channel);case TransmitOutcome_CourierQueued():
return courierQueued();case TransmitOutcome_PlainReady():
return plainReady(_that.op,_that.text);case TransmitOutcome_ConsentRequired():
return consentRequired(_that.field0);case TransmitOutcome_Rejected():
return rejected(_that.reason);}
}
/// A variant of `when` that fallback to returning `null`
///
/// It is equivalent to doing:
/// ```dart
/// switch (sealedClass) {
///   case Subclass(:final field):
///     return ...;
///   case _:
///     return null;
/// }
/// ```

@optionalTypeArgs TResult? whenOrNull<TResult extends Object?>({TResult? Function( BigInt op,  Channel channel,  String text,  QrAnimation? animation)?  ready,TResult? Function( Channel channel)?  sending,TResult? Function()?  courierQueued,TResult? Function( BigInt op,  String text)?  plainReady,TResult? Function( ConsentRequest field0)?  consentRequired,TResult? Function( String reason)?  rejected,}) {final _that = this;
switch (_that) {
case TransmitOutcome_Ready() when ready != null:
return ready(_that.op,_that.channel,_that.text,_that.animation);case TransmitOutcome_Sending() when sending != null:
return sending(_that.channel);case TransmitOutcome_CourierQueued() when courierQueued != null:
return courierQueued();case TransmitOutcome_PlainReady() when plainReady != null:
return plainReady(_that.op,_that.text);case TransmitOutcome_ConsentRequired() when consentRequired != null:
return consentRequired(_that.field0);case TransmitOutcome_Rejected() when rejected != null:
return rejected(_that.reason);case _:
  return null;

}
}

}

/// @nodoc


class TransmitOutcome_Ready extends TransmitOutcome {
  const TransmitOutcome_Ready({required this.op, required this.channel, required this.text, this.animation}): super._();
  

/// Operation id.
 final  BigInt op;
/// Channel.
 final  Channel channel;
/// Armored bundle text (share channel).
 final  String text;
/// Animated QR frames (QR channel).
 final  QrAnimation? animation;

/// Create a copy of TransmitOutcome
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$TransmitOutcome_ReadyCopyWith<TransmitOutcome_Ready> get copyWith => _$TransmitOutcome_ReadyCopyWithImpl<TransmitOutcome_Ready>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is TransmitOutcome_Ready&&(identical(other.op, op) || other.op == op)&&(identical(other.channel, channel) || other.channel == channel)&&(identical(other.text, text) || other.text == text)&&(identical(other.animation, animation) || other.animation == animation));
}


@override
int get hashCode {
    return Object.hash(runtimeType,op,channel,text,animation);
}

@override
String toString() {
    return 'TransmitOutcome.ready(op: $op, channel: $channel, text: $text, animation: $animation)';
}


}

/// @nodoc
abstract mixin class $TransmitOutcome_ReadyCopyWith<$Res> implements $TransmitOutcomeCopyWith<$Res> {
  factory $TransmitOutcome_ReadyCopyWith(TransmitOutcome_Ready value, $Res Function(TransmitOutcome_Ready) _then) = _$TransmitOutcome_ReadyCopyWithImpl;
@useResult
$Res call({
 BigInt op, Channel channel, String text, QrAnimation? animation
});




}
/// @nodoc
class _$TransmitOutcome_ReadyCopyWithImpl<$Res>
    implements $TransmitOutcome_ReadyCopyWith<$Res> {
  _$TransmitOutcome_ReadyCopyWithImpl(this._self, this._then);

  final TransmitOutcome_Ready _self;
  final $Res Function(TransmitOutcome_Ready) _then;

/// Create a copy of TransmitOutcome
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? op = null,Object? channel = null,Object? text = null,Object? animation = freezed,}) {
  return _then(TransmitOutcome_Ready(
op: null == op ? _self.op : op // ignore: cast_nullable_to_non_nullable
as BigInt,channel: null == channel ? _self.channel : channel // ignore: cast_nullable_to_non_nullable
as Channel,text: null == text ? _self.text : text // ignore: cast_nullable_to_non_nullable
as String,animation: freezed == animation ? _self.animation : animation // ignore: cast_nullable_to_non_nullable
as QrAnimation?,
  ));
}


}

/// @nodoc


class TransmitOutcome_Sending extends TransmitOutcome {
  const TransmitOutcome_Sending({required this.channel}): super._();
  

/// Channel.
 final  Channel channel;

/// Create a copy of TransmitOutcome
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$TransmitOutcome_SendingCopyWith<TransmitOutcome_Sending> get copyWith => _$TransmitOutcome_SendingCopyWithImpl<TransmitOutcome_Sending>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is TransmitOutcome_Sending&&(identical(other.channel, channel) || other.channel == channel));
}


@override
int get hashCode {
    return Object.hash(runtimeType,channel);
}

@override
String toString() {
    return 'TransmitOutcome.sending(channel: $channel)';
}


}

/// @nodoc
abstract mixin class $TransmitOutcome_SendingCopyWith<$Res> implements $TransmitOutcomeCopyWith<$Res> {
  factory $TransmitOutcome_SendingCopyWith(TransmitOutcome_Sending value, $Res Function(TransmitOutcome_Sending) _then) = _$TransmitOutcome_SendingCopyWithImpl;
@useResult
$Res call({
 Channel channel
});




}
/// @nodoc
class _$TransmitOutcome_SendingCopyWithImpl<$Res>
    implements $TransmitOutcome_SendingCopyWith<$Res> {
  _$TransmitOutcome_SendingCopyWithImpl(this._self, this._then);

  final TransmitOutcome_Sending _self;
  final $Res Function(TransmitOutcome_Sending) _then;

/// Create a copy of TransmitOutcome
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? channel = null,}) {
  return _then(TransmitOutcome_Sending(
channel: null == channel ? _self.channel : channel // ignore: cast_nullable_to_non_nullable
as Channel,
  ));
}


}

/// @nodoc


class TransmitOutcome_CourierQueued extends TransmitOutcome {
  const TransmitOutcome_CourierQueued(): super._();
  






@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is TransmitOutcome_CourierQueued);
}


@override
int get hashCode => runtimeType.hashCode;

@override
String toString() {
    return 'TransmitOutcome.courierQueued()';
}


}




/// @nodoc


class TransmitOutcome_PlainReady extends TransmitOutcome {
  const TransmitOutcome_PlainReady({required this.op, required this.text}): super._();
  

/// Operation id.
 final  BigInt op;
/// The exact text that will be exposed.
 final  String text;

/// Create a copy of TransmitOutcome
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$TransmitOutcome_PlainReadyCopyWith<TransmitOutcome_PlainReady> get copyWith => _$TransmitOutcome_PlainReadyCopyWithImpl<TransmitOutcome_PlainReady>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is TransmitOutcome_PlainReady&&(identical(other.op, op) || other.op == op)&&(identical(other.text, text) || other.text == text));
}


@override
int get hashCode {
    return Object.hash(runtimeType,op,text);
}

@override
String toString() {
    return 'TransmitOutcome.plainReady(op: $op, text: $text)';
}


}

/// @nodoc
abstract mixin class $TransmitOutcome_PlainReadyCopyWith<$Res> implements $TransmitOutcomeCopyWith<$Res> {
  factory $TransmitOutcome_PlainReadyCopyWith(TransmitOutcome_PlainReady value, $Res Function(TransmitOutcome_PlainReady) _then) = _$TransmitOutcome_PlainReadyCopyWithImpl;
@useResult
$Res call({
 BigInt op, String text
});




}
/// @nodoc
class _$TransmitOutcome_PlainReadyCopyWithImpl<$Res>
    implements $TransmitOutcome_PlainReadyCopyWith<$Res> {
  _$TransmitOutcome_PlainReadyCopyWithImpl(this._self, this._then);

  final TransmitOutcome_PlainReady _self;
  final $Res Function(TransmitOutcome_PlainReady) _then;

/// Create a copy of TransmitOutcome
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? op = null,Object? text = null,}) {
  return _then(TransmitOutcome_PlainReady(
op: null == op ? _self.op : op // ignore: cast_nullable_to_non_nullable
as BigInt,text: null == text ? _self.text : text // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

/// @nodoc


class TransmitOutcome_ConsentRequired extends TransmitOutcome {
  const TransmitOutcome_ConsentRequired(this.field0): super._();
  

 final  ConsentRequest field0;

/// Create a copy of TransmitOutcome
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$TransmitOutcome_ConsentRequiredCopyWith<TransmitOutcome_ConsentRequired> get copyWith => _$TransmitOutcome_ConsentRequiredCopyWithImpl<TransmitOutcome_ConsentRequired>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is TransmitOutcome_ConsentRequired&&(identical(other.field0, field0) || other.field0 == field0));
}


@override
int get hashCode {
    return Object.hash(runtimeType,field0);
}

@override
String toString() {
    return 'TransmitOutcome.consentRequired(field0: $field0)';
}


}

/// @nodoc
abstract mixin class $TransmitOutcome_ConsentRequiredCopyWith<$Res> implements $TransmitOutcomeCopyWith<$Res> {
  factory $TransmitOutcome_ConsentRequiredCopyWith(TransmitOutcome_ConsentRequired value, $Res Function(TransmitOutcome_ConsentRequired) _then) = _$TransmitOutcome_ConsentRequiredCopyWithImpl;
@useResult
$Res call({
 ConsentRequest field0
});




}
/// @nodoc
class _$TransmitOutcome_ConsentRequiredCopyWithImpl<$Res>
    implements $TransmitOutcome_ConsentRequiredCopyWith<$Res> {
  _$TransmitOutcome_ConsentRequiredCopyWithImpl(this._self, this._then);

  final TransmitOutcome_ConsentRequired _self;
  final $Res Function(TransmitOutcome_ConsentRequired) _then;

/// Create a copy of TransmitOutcome
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? field0 = null,}) {
  return _then(TransmitOutcome_ConsentRequired(
null == field0 ? _self.field0 : field0 // ignore: cast_nullable_to_non_nullable
as ConsentRequest,
  ));
}


}

/// @nodoc


class TransmitOutcome_Rejected extends TransmitOutcome {
  const TransmitOutcome_Rejected({required this.reason}): super._();
  

/// Reason key.
 final  String reason;

/// Create a copy of TransmitOutcome
/// with the given fields replaced by the non-null parameter values.
@JsonKey(includeFromJson: false, includeToJson: false)
@pragma('vm:prefer-inline')
$TransmitOutcome_RejectedCopyWith<TransmitOutcome_Rejected> get copyWith => _$TransmitOutcome_RejectedCopyWithImpl<TransmitOutcome_Rejected>(this, _$identity);



@override
bool operator ==(Object other) {
    return identical(this, other) || (other.runtimeType == runtimeType&&other is TransmitOutcome_Rejected&&(identical(other.reason, reason) || other.reason == reason));
}


@override
int get hashCode {
    return Object.hash(runtimeType,reason);
}

@override
String toString() {
    return 'TransmitOutcome.rejected(reason: $reason)';
}


}

/// @nodoc
abstract mixin class $TransmitOutcome_RejectedCopyWith<$Res> implements $TransmitOutcomeCopyWith<$Res> {
  factory $TransmitOutcome_RejectedCopyWith(TransmitOutcome_Rejected value, $Res Function(TransmitOutcome_Rejected) _then) = _$TransmitOutcome_RejectedCopyWithImpl;
@useResult
$Res call({
 String reason
});




}
/// @nodoc
class _$TransmitOutcome_RejectedCopyWithImpl<$Res>
    implements $TransmitOutcome_RejectedCopyWith<$Res> {
  _$TransmitOutcome_RejectedCopyWithImpl(this._self, this._then);

  final TransmitOutcome_Rejected _self;
  final $Res Function(TransmitOutcome_Rejected) _then;

/// Create a copy of TransmitOutcome
/// with the given fields replaced by the non-null parameter values.
@pragma('vm:prefer-inline') $Res call({Object? reason = null,}) {
  return _then(TransmitOutcome_Rejected(
reason: null == reason ? _self.reason : reason // ignore: cast_nullable_to_non_nullable
as String,
  ));
}


}

// dart format on
