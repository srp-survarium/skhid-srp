package survarium.controls
{
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.text.TextField;
   import flash.utils.Dictionary;
   import survarium.Util;
   import survarium.core.Icon;
   import survarium.data.PropertiesDescriptor;
   import survarium.item.ItemFactory;
   
   public class ItemCompare extends MovieClip
   {
      
      protected static var _propsMin:Dictionary = new Dictionary();
      
      protected static var _propsMax:Dictionary = new Dictionary();
      
      protected static var _propsDir:Dictionary = new Dictionary();
      
      protected static var _propsDescriptor:PropertiesDescriptor;
      
      public var diff_1:MovieClip;
      
      public var diff_2:MovieClip;
      
      public var diff_3:MovieClip;
      
      public var diff_4:MovieClip;
      
      public var diff_5:MovieClip;
      
      public var diff_6:MovieClip;
      
      public var diff_7:MovieClip;
      
      public var diff_8:MovieClip;
      
      public var textFieldGun:TextField;
      
      protected var _primary:Object;
      
      protected var _secondary:Object;
      
      protected var _spriteAnchor:Sprite;
      
      protected var _indicator:Array;
      
      public function ItemCompare()
      {
         var _loc2_:MovieClip = null;
         this._spriteAnchor = new Sprite();
         super();
         this._spriteAnchor.x -= 130;
         this._spriteAnchor.y += 120;
         this._indicator = [this.diff_1,this.diff_2,this.diff_3,this.diff_4,this.diff_5,this.diff_6,this.diff_7,this.diff_8];
         _loc2_ = MovieClip(this.getChildByName("diff_9"));
         if(_loc2_)
         {
            this._indicator.public::push(_loc2_);
         }
         var _loc1_:int = 0;
         while(_loc1_ < this._indicator.length)
         {
            _loc2_ = this._indicator[_loc1_];
            _loc2_.visible = false;
            _loc2_.background.visible = false;
            _loc1_++;
         }
      }
      
      public static function setItemsMaxValues(param1:Array) : void
      {
         var _loc2_:int = 0;
         while(_loc2_ < param1.length)
         {
            _propsMax[param1[_loc2_].icon_id] = param1[_loc2_].max;
            _propsMin[param1[_loc2_].icon_id] = param1[_loc2_].min;
            _propsDir[param1[_loc2_].icon_id] = param1[_loc2_].direction;
            _loc2_++;
         }
      }
      
      public static function setPropsDescriptor(param1:PropertiesDescriptor) : void
      {
         trace("setPropsDescriptor::",param1);
         _propsDescriptor = param1;
      }
      
      public function fillModifiers(param1:Object) : void
      {
         var _loc2_:Array = param1.modifiers;
         if(!_loc2_)
         {
            return;
         }
         var _loc3_:Object = new Object();
         var _loc4_:Object = new Object();
         var _loc5_:uint = 0;
         var _loc6_:Object;
         var _loc7_:Object;
         var _loc8_:int;
         var _loc9_:Number;
         while(_loc5_ < _loc2_.length)
         {
            _loc6_ = _loc2_[_loc5_].props;
            for each(_loc7_ in _loc6_)
            {
               _loc8_ = _loc7_.prop_id;
               if(_loc4_[_loc8_] == undefined)
               {
                  _loc4_[_loc8_] = {
                     prop_id:_loc8_,
                     prop_value:0
                  };
               }
               _loc9_ = Number(_loc7_.prop_value);
               if(_loc9_ != Number(_loc7_.prop_value))
               {
                  _loc9_ = Util.stringToNumber(_loc7_.prop_value);
               }
               _loc4_[_loc8_].prop_value += _loc9_;
            }
            _loc5_++;
         }
         _loc3_.props = _loc4_;
         this.setModifiers(_loc3_);
      }
      
      public function setModifiers(param1:Object) : void
      {
         var _loc2_:uint = this._indicator.length;
         var _loc3_:Object;
         var _loc4_:uint;
         var _loc5_:MovieClip;
         var _loc6_:Object;
         var _loc7_:Number;
         var _loc8_:Number;
         if(param1)
         {
            for each(_loc3_ in param1.props)
            {
               _loc4_ = 0;
               while(_loc4_ < _loc2_)
               {
                  _loc5_ = this._indicator[_loc4_];
                  if(_loc5_.id == _loc3_.prop_id)
                  {
                     _loc6_ = _propsDescriptor.getPropertyDescription(_loc3_.prop_id);
                     _loc7_ = _loc5_.indicator.value - _loc5_.indicator.extra;
                     _loc8_ = _loc6_.modifier_type == 0 ? _loc3_.prop_value : _loc7_ * _loc3_.prop_value;
                     trace("_diff.indicator.value::",_loc6_.modifier_type,_loc7_,_loc5_.indicator.value,_loc3_.prop_value,_loc8_);
                     _loc5_.indicator.upgrade = _loc8_;
                     break;
                  }
                  _loc4_++;
               }
            }
         }
         else
         {
            _loc4_ = 0;
            while(_loc4_ < _loc2_)
            {
               _loc5_ = this._indicator[_loc4_];
               _loc5_.indicator.upgrade = 0;
               _loc4_++;
            }
         }
      }
      
      public function setPrimaryWeapon(param1:Object) : void
      {
         _primary = param1;
         this.updateData();
      }
      
      public function getPrimaryData() : Object
      {
         return this._primary;
      }
      
      public function setSecondaryWeapon(param1:Object) : void
      {
         if(this._secondary != param1 && this._primary != param1)
         {
            _secondary = param1;
            this.updateData();
         }
      }
      
      public function setIcon(param1:Sprite) : void
      {
         if(this._spriteAnchor.numChildren)
         {
            this._spriteAnchor.removeChildAt(0);
         }
         if(this._primary.dragIcon is Icon)
         {
            this._spriteAnchor.addChild(this._primary.dragIcon.clone());
         }
      }
      
      protected function updateData() : void
      {
         if(!_propsDescriptor)
         {
            trace("!props_descriptor::");
            return;
         }
         if(!this._primary)
         {
            return;
         }
         trace("ItemCompare::updateData");
         var prim_cat:int = ItemFactory.getCategory(this._primary.category);
         var index:int = 0;
         var _diff:MovieClip = null;
         var propDescription:Object = null;
         var indicators:uint = this._indicator.length;
         var modifier_value:Number = 0;
         function initPropValues(param1:Object, param2:Object):void
         {
            modifier_value = 0;
            var _loc3_:Number = 0;
            var _loc4_:int = 0;
            var _loc5_:Object;
            var _loc6_:Object;
            var _loc7_:Number;
            var _loc8_:Object;
            if(param2)
            {
               for each(_loc5_ in param2)
               {
                  for each(_loc6_ in _loc5_.props)
                  {
                     if(_loc6_.prop_id == propDescription.prop_id)
                     {
                        _loc7_ = Number(_loc6_.prop_value);
                        if(_loc7_ != Number(_loc6_.prop_value))
                        {
                           _loc7_ = Util.stringToNumber(String(_loc6_.prop_value));
                        }
                        modifier_value += _loc7_;
                        _loc8_ = _propsDescriptor.getPropertyDescription(_loc6_.prop_id);
                        _loc4_ = _loc8_.modifier_type;
                     }
                  }
               }
            }
            if(_loc4_ == 0)
            {
               _loc3_ = param1.prop_value + modifier_value;
            }
            else
            {
               _loc3_ = param1.prop_value * (1 + modifier_value);
            }
            if(_loc4_ == 1)
            {
               modifier_value = param1.prop_value * modifier_value;
            }
            _value = Number(_loc3_);
            if(_value != Number(_loc3_))
            {
               _value = Util.stringToNumber(String(_loc3_));
            }
         }
         function initIndicatorValues(param1:MovieClip, param2:Object, param3:Number, param4:Number):void
         {
            _diff = param1;
            _diff.visible = true;
            addChild(_diff);
            _diff.indicator.direction = param2.direction;
            _diff.indicator.maximum = param2.max_value;
            _diff.indicator.minimum = param2.min_value;
            _diff.indicator.label = param2.prop_name;
            _diff.indicator.upgrade = 0;
            _diff.id = param2.prop_id;
            if(param2.postfix != undefined)
            {
               _diff.indicator.postfix = param2.postfix;
            }
            else
            {
               _diff.indicator.postfix = "";
            }
            _diff.indicator.extra = modifier_value;
            _diff.indicator.compare = param3;
            _diff.indicator.value = param4;
         }
         for each(var s:Object in this._primary.properties)
         {
            var _valueObject:Object = s;
            propDescription = _propsDescriptor.getPropertyDescription(_valueObject.prop_id);
            if(!propDescription)
            {
               trace("wrong propDescription",_valueObject.prop_id);
            }
            else
            {
               var _value:Number = 0;
               trace("primaryProps::",propDescription.prop_name,"value::",_valueObject.prop_value);
               if(propDescription.comparable)
               {
                  initPropValues(_valueObject,this._primary.modifiers);
                  initIndicatorValues(this._indicator[index],propDescription,_value,_value);
                  index++;
                  if(index == indicators)
                  {
                     break;
                  }
               }
            }
         }
         while(index < indicators)
         {
            _diff = this._indicator[index];
            removeChild(_diff);
            _diff.visible = false;
            index++;
         }
         if(!this._secondary || (prim_cat == 2 ? (prim_cat != ItemFactory.getCategory(this._secondary.category)) : this._primary.category != this._secondary.category))
         {
            dispatchEvent(new Event(Event.RESIZE));
            return;
         }
         index = 0;
         while(index < indicators)
         {
            _diff = this._indicator[index];
            if(_diff.visible)
            {
               _diff.indicator.compare = 0;
            }
            index++;
         }
         for each(s in this._secondary.properties)
         {
            _valueObject = s;
            propDescription = _propsDescriptor.getPropertyDescription(_valueObject.prop_id);
            if(!propDescription)
            {
               trace("wrong propDescription",_valueObject.prop_id,propDescription);
            }
            else if(propDescription.comparable)
            {
               initPropValues(_valueObject,this._secondary.modifiers);
               index = 0;
               while(index < indicators)
               {
                  _diff = this._indicator[index];
                  if(!_diff.visible)
                  {
                     initIndicatorValues(this._indicator[index],propDescription,_value,0);
                     break;
                  }
                  else if(_diff.id == propDescription.prop_id)
                  {
                     _diff.indicator.compare = _value;
                     _diff.indicator.extra = 0;
                     break;
                  }
                  index++;
               }
            }
         }
         dispatchEvent(new Event(Event.RESIZE));
      }
   }
}

