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
   import survarium.item.ModificationRolls;
   import survarium.item.WeaponDetails;
   import survarium.player.LocalPlayerProfile;
   import survarium.player.MatchEffects;
   import survarium.constants.ModifierType;
   import survarium.constants.SuperCategory;
   import survarium.constants.ItemCategory;
   
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
         var diff:MovieClip = null;
         this._spriteAnchor = new Sprite();
         super();
         this._spriteAnchor.x -= 130;
         this._spriteAnchor.y += 120;
         this._indicator = [this.diff_1,this.diff_2,this.diff_3,this.diff_4,this.diff_5,this.diff_6,this.diff_7,this.diff_8];
         diff = MovieClip(this.getChildByName("diff_9"));
         if(diff)
         {
            this._indicator.public::push(diff);
         }
         var i:int = 0;
         while(i < this._indicator.length)
         {
            diff = this._indicator[i];
            diff.visible = false;
            diff.background.visible = false;
            i++;
         }
      }
      
      public static function setItemsMaxValues(limits:Array) : void
      {
         var i:int = 0;
         while(i < limits.length)
         {
            _propsMax[limits[i].icon_id] = limits[i].max;
            _propsMin[limits[i].icon_id] = limits[i].min;
            _propsDir[limits[i].icon_id] = limits[i].direction;
            i++;
         }
      }
      
      // A card property a modifier of another property changes: a weapon's aim-time roll (102,
      // 2070..2074) scales its aim time (133, parameters/aim_time) since weapon-rolls-1.
      private static const MODIFIED_BY:Object = {133:102};

      // Whether a modifier of `modifierProp` changes the card property `prop`.
      private static function modifies(modifierProp:int, prop:int) : Boolean
      {
         return modifierProp == prop || MODIFIED_BY[prop] === modifierProp;
      }

      public static function setPropsDescriptor(descriptor:PropertiesDescriptor) : void
      {
         trace("setPropsDescriptor::",descriptor);
         _propsDescriptor = descriptor;
      }
      
      public function fillModifiers(item:Object) : void
      {
         var modifiers:Array = item.modifiers;
         if(!modifiers)
         {
            return;
         }
         var combined:Object = new Object();
         var propTotals:Object = new Object();
         var i:uint = 0;
         var props:Object;
         var prop:Object;
         var propId:int;
         var propValue:Number;
         while(i < modifiers.length)
         {
            props = modifiers[i].props;
            for each(prop in props)
            {
               propId = prop.prop_id;
               if(propTotals[propId] == undefined)
               {
                  propTotals[propId] = {
                     prop_id:propId,
                     prop_value:0
                  };
               }
               propValue = Number(prop.prop_value);
               if(propValue != Number(prop.prop_value))
               {
                  propValue = Util.stringToNumber(prop.prop_value);
               }
               propTotals[propId].prop_value += propValue;
            }
            i++;
         }
         combined.props = propTotals;
         this.setModifiers(combined);
      }
      
      public function setModifiers(modifiers:Object) : void
      {
         var count:uint = this._indicator.length;
         var prop:Object;
         var i:uint;
         var diff:MovieClip;
         var description:Object;
         var baseValue:Number;
         var upgradeValue:Number;
         if(modifiers)
         {
            for each(prop in modifiers.props)
            {
               i = 0;
               while(i < count)
               {
                  diff = this._indicator[i];
                  if(diff.id == prop.prop_id)
                  {
                     description = _propsDescriptor.getPropertyDescription(prop.prop_id);
                     baseValue = diff.indicator.value - diff.indicator.extra;
                     upgradeValue = description.modifier_type == ModifierType.ADD ? prop.prop_value : baseValue * prop.prop_value;
                     trace("_diff.indicator.value::",description.modifier_type,baseValue,diff.indicator.value,prop.prop_value,upgradeValue);
                     diff.indicator.upgrade = upgradeValue;
                     break;
                  }
                  i++;
               }
            }
         }
         else
         {
            i = 0;
            while(i < count)
            {
               diff = this._indicator[i];
               diff.indicator.upgrade = 0;
               i++;
            }
         }
      }
      
      public function setPrimaryWeapon(item:Object) : void
      {
         _primary = item;
         this.updateData();
      }
      
      public function getPrimaryData() : Object
      {
         return this._primary;
      }

      public function getSecondaryData() : Object { return this._secondary; }

      public function refreshComparison() : void { updateData(); }
      
      public function setSecondaryWeapon(item:Object) : void
      {
         if(this._secondary != item && this._primary != item)
         {
            _secondary = item;
            this.updateData();
         }
      }
      
      public function setIcon(icon:Sprite) : void
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
         var medical:Boolean = this._primary.category == ItemCategory.MEDKITS;
         var weaponDetails:Boolean = WeaponDetails.read(this._primary) != null;
         var index:int = 0;
         var _diff:MovieClip = null;
         var propDescription:Object = null;
         var indicators:uint = this._indicator.length;
         var modifier_value:Number = 0;
         // Held Alt over a weapon: the stats as a match has them, with the kit and the skills.
         var effects:MatchEffects = LocalPlayerProfile.heldMatchEffects(this._primary);
         // The weapon's own base value of the property last read (initPropValues).
         var base_value:Number = 0;
         function initPropValues(baseProperty:Object, modifierList:Object):void
         {
            modifier_value = 0;
            var total:Number = 0;
            var modifierType:int = 0;
            var modifierEntry:Object;
            var modifierProp:Object;
            var propValue:Number;
            var modifierDescription:Object;
            if(modifierList)
            {
               for each(modifierEntry in modifierList)
               {
                  for each(modifierProp in modifierEntry.props)
                  {
                     if(modifies(modifierProp.prop_id,propDescription.prop_id))
                     {
                        propValue = Number(modifierProp.prop_value);
                        if(propValue != Number(modifierProp.prop_value))
                        {
                           propValue = Util.stringToNumber(String(modifierProp.prop_value));
                        }
                        modifier_value += propValue;
                        modifierDescription = _propsDescriptor.getPropertyDescription(modifierProp.prop_id);
                        modifierType = modifierDescription.modifier_type;
                     }
                  }
               }
            }
            // The exe sends a line's prop_value as the config's text ("175", ui_desc/props_list):
            // a number before any sum, where "175" + 0 would be "1750".
            var base:Number = ModificationRolls.value(baseProperty);
            base_value = base;
            if(modifierType == ModifierType.ADD)
            {
               total = base + modifier_value;
            }
            else
            {
               // The cook's type 2: base * max(0, 1 + sum) (0x358010).
               total = base * Math.max(0,1 + modifier_value);
            }
            if(modifierType == ModifierType.MULTIPLY)
            {
               modifier_value = base * modifier_value;
            }
            _value = effects == null ? total : effects.stat(propDescription.prop_id,total);
         }
         function initIndicatorValues(diffClip:MovieClip, propertyInfo:Object, compareValue:Number, currentValue:Number):void
         {
            _diff = diffClip;
            _diff.visible = true;
            addChild(_diff);
            _diff.indicator.direction = propertyInfo.direction;
            _diff.indicator.maximum = propertyInfo.max_value;
            _diff.indicator.minimum = propertyInfo.min_value;
            _diff.indicator.label = propertyInfo.prop_name;
            _diff.indicator.upgrade = 0;
            _diff.id = propertyInfo.prop_id;
            if(propertyInfo.postfix != undefined)
            {
               _diff.indicator.postfix = propertyInfo.postfix;
            }
            else
            {
               _diff.indicator.postfix = "";
            }
            _diff.indicator.extra = modifier_value;
            _diff.indicator.compare = compareValue;
            _diff.indicator.value = currentValue;
            _diff.indicator.changeText = null;
         }
         for each(var s:Object in this._primary.properties)
         {
            if(weaponDetails && s.prop_id == 24) continue;
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
                  // While Alt is held, a value its modifications, the kit or the skills change
                  // reads "base -> in a match".
                  if(effects != null && MatchEffects.changed(base_value,_value))
                     _diff.indicator.changeText = MatchEffects.change(base_value,_value,2) + _diff.indicator.postfix;
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
            if(_diff.parent == this) removeChild(_diff);
            _diff.visible = false;
            index++;
         }
         if(!this._secondary || (prim_cat == SuperCategory.WEAPONS ? (prim_cat != ItemFactory.getCategory(this._secondary.category)) : this._primary.category != this._secondary.category))
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
               // A medicine without this property has no comparable value.
               // Missing healing is not a claim of zero healing.
               _diff.indicator.compare = medical ? _diff.indicator.value : 0;
            }
            index++;
         }
         for each(s in this._secondary.properties)
         {
            if(weaponDetails && s.prop_id == 24) continue;
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
                     if(medical) break;
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

