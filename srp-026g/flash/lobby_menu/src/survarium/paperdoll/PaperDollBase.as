package survarium.paperdoll
{
   import flash.events.Event;
   import flash.events.MouseEvent;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.controls.DragSlot;
   import scaleform.clik.controls.ToolTip;
   import scaleform.clik.core.UIComponent;
   import scaleform.clik.events.ListEvent;
   import scaleform.clik.interfaces.IDataProvider;
   import scaleform.clik.interfaces.IListItemRenderer;
   import survarium.constants.Category;
   import survarium.data.ItemData;
   import survarium.data.LocalizationData;
   import survarium.interfaces.IPaperDoll;
   import survarium.interfaces.IPaperDollSlot;
   
   public class PaperDollBase extends UIComponent implements IPaperDoll
   {
      
      public static const PAPERDOLL_UPDATED:String = "paperDollUpdated";
      
      protected static var _slotNames:Array = ["slotHead","slotMask","slotBody","slotBack","slotLegs","slotArms","slotFoot","slotWpn1","slotWpn1A1","slotWpn1A2","slotWpn2","slotWpn2A1","slotWpn2A2","slotQck1","slotQck2","slotQck3","slotQck4","slotQck5","slotQck6","slotWpn5","slotWpn6","slotWpn7","slotWpn8"];
      
      protected var _slots:Array = [];
      
      protected var _dataProvider:IDataProvider;
      
      protected var _slotTypes:Array = ["head","mask","torso","back","legs","hands","foot","weapon","ammo","ammo","weapon","ammo","ammo","slot","slot","slot","slot","slot","slot"];
      
      protected var _slotLabels:Array = ["filter_helmet","filter_mask","filter_body_armor","filter_rig","filter_pants","filter_gloves","filter_boots","pd_weapon","pd_ammo","pd_ammo","pd_weapon","pd_ammo","pd_ammo","pd_artifact","pd_artifact","pd_artifact","pd_artifact","pd_artifact","pd_artifact","pd_ammo","pd_ammo","pd_ammo","pd_ammo"];
      
      protected var _slotsN:int = 0;
      
      protected var _slotsA:int = 0;
      
      public function PaperDollBase()
      {
         super();
      }
      
      override protected function initialize() : void
      {
         super.initialize();
         _slots = [];
         tabEnabled = tabChildren = false;
      }
      
      public function get dataProvider() : IDataProvider
      {
         return this._dataProvider;
      }
      
      public function set dataProvider(param1:IDataProvider) : void
      {
         if(this._dataProvider == param1)
         {
            return;
         }
         if(this._dataProvider != null)
         {
            this._dataProvider.removeEventListener(Event.CHANGE,this.handleDataChange,false);
         }
         _dataProvider = param1;
         if(this._dataProvider == null)
         {
            return;
         }
         this._dataProvider.addEventListener(Event.CHANGE,this.handleDataChange,false,0,true);
         invalidateData();
      }
      
      public function getSlotAt(param1:int) : IPaperDollSlot
      {
         return this._slots[param1];
      }
      
      public function getSlotByName(param1:String) : int
      {
         var _loc2_:int = _slotNames.indexOf(param1);
         if(_loc2_ == -1)
         {
            _loc2_ = 1;
         }
         return this._dataProvider[_loc2_];
      }
      
      override public function toString() : String
      {
         return "[PaperDollBase " + name + "]";
      }
      
      public function openSlots(param1:int, param2:int) : void
      {
         _slotsN = param1;
         _slotsA = param2;
         invalidateData();
      }
      
      protected function handleDataChange(param1:Event) : void
      {
         invalidate(InvalidationType.DATA);
      }
      
      protected function updateSlotLabels(param1:String) : void
      {
         var _loc2_:uint = 0;
         var _loc3_:IPaperDollSlot;
         for(; _loc2_ < _slotNames.length; _loc2_++)
         {
            _loc3_ = getChildByName(_slotNames[_loc2_]) as IPaperDollSlot;
            if(!_loc3_)
            {
               continue;
            }
            _loc3_.label = LocalizationData.getName(this._slotLabels[_loc2_]);
         }
      }
      
      protected function configSlot(param1:IPaperDollSlot) : void
      {
         (param1 as DragSlot).dragEnabled = false;
         (param1 as ToolTip).tooltipRendererName = "SrvItemHintRednerer";
      }
      
      protected function handleSlotListeners(param1:IPaperDollSlot) : void
      {
         param1.addEventListener(MouseEvent.ROLL_OVER,this.newItemOver);
         param1.addEventListener(MouseEvent.ROLL_OUT,this.newItemOut);
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         LocalizationData.addCallback(this.updateSlotLabels,"sell");
         var _loc1_:uint = 0;
         var _loc2_:IPaperDollSlot;
         for(; _loc1_ < _slotNames.length; _loc1_++)
         {
            _loc2_ = getChildByName(_slotNames[_loc1_]) as IPaperDollSlot;
            if(!_loc2_)
            {
               continue;
            }
            _loc2_.index = _loc1_;
            _loc2_.owner = this;
            _loc2_.selected = false;
            _loc2_.label = LocalizationData.getName(this._slotLabels[_loc1_]);
            this._slots[_loc1_] = _loc2_;
            this.handleSlotListeners(_loc2_);
            this.configSlot(_loc2_);
         }
      }
      
      protected function sortAmmo() : void
      {
      }
      
      override protected function draw() : void
      {
         var _loc1_:Array;
         var _loc2_:IDataProvider;
         var _loc3_:uint;
         var _loc4_:uint;
         var _loc5_:uint;
         var _loc6_:uint;
         var _loc7_:ItemData;
         var _loc8_:IPaperDollSlot;
         var _loc9_:ItemData;
         var _loc10_:Boolean;
         super.draw();
         if(isInvalid(InvalidationType.DATA))
         {
            _loc1_ = new Array();
            this.sortAmmo();
            _loc2_ = this._dataProvider;
            if(this._dataProvider != null)
            {
               _loc3_ = _loc2_.length;
               _loc4_ = 0;
               while(_loc4_ < _loc3_)
               {
                  _loc9_ = _loc2_[_loc4_];
                  if(_loc9_.additionalSlots)
                  {
                     _loc6_ = 0;
                     while(_loc6_ < _loc9_.additionalSlots.length)
                     {
                        _loc7_ = _loc9_.clone();
                        _loc7_.additionalSlots = null;
                        _loc7_.enabled = false;
                        _loc7_.slotId = _loc9_.additionalSlots[_loc6_];
                        _loc1_[_loc7_.slotId] = _loc7_;
                        _loc6_++;
                     }
                  }
                  _loc4_++;
               }
               _loc4_ = 0;
               while(_loc4_ < _loc3_)
               {
                  trace("newItemsL",_loc4_,_loc2_[_loc4_].name,_loc2_[_loc4_].slotId);
                  _loc1_[_loc2_[_loc4_].slotId] = _loc2_[_loc4_];
                  _loc4_++;
               }
               _loc5_ = this._slots.length;
               _loc4_ = 0;
               for(; _loc4_ < _loc5_; _loc4_++)
               {
                  _loc8_ = this._slots[_loc4_] as IPaperDollSlot;
                  trace("DRAW_PDB",_loc4_,_loc8_,_loc1_[_loc4_]);
                  if(!_loc8_)
                  {
                     continue;
                  }
                  _loc9_ = _loc1_[_loc4_];
                  _loc10_ = _loc9_ && _loc9_.superCategory == Category.BASE_AMMO;
                  if(_loc9_ && (_loc10_ && _loc9_.value != 0 || !_loc10_))
                  {
                     _loc8_.setData(_loc9_);
                     _loc8_.enabled = _loc9_.enabled;
                  }
                  else
                  {
                     _loc8_.setData(null);
                     _loc8_.enabled = true;
                  }
                  _loc8_.invalidateData();
                  _loc8_.validateNow();
               }
            }
            this.handleQuickSlots();
            dispatchEvent(new Event(PAPERDOLL_UPDATED));
         }
      }
      
      protected function handleQuickSlots() : void
      {
         var _loc1_:int = this._slotsN + this._slotsA;
         var _loc2_:uint = 1;
         var _loc3_:IPaperDollSlot;
         while(_loc2_ < 7)
         {
            _loc3_ = this._slots[_loc2_ + 12];
            trace("handleQuickSlots",_loc3_);
            if(_loc2_ > _loc1_)
            {
               if(_loc3_.data)
               {
                  _loc3_.dropSlot();
               }
               _loc3_.enabled = false;
               _loc3_.artifact = false;
               _loc3_.visible = false;
            }
            else
            {
               _loc3_.visible = true;
               if(_loc2_ > this._slotsN)
               {
                  _loc3_.label = LocalizationData.getName("pd_artifact");
                  _loc3_.artifact = true;
                  if(_loc3_.data)
                  {
                     _loc3_.dropSlot();
                  }
                  _loc3_.enabled = false;
               }
               else
               {
                  _loc3_.label = LocalizationData.getName("filter_items");
                  _loc3_.artifact = false;
                  _loc3_.enabled = true;
               }
            }
            _loc2_++;
         }
      }
      
      protected function newItemOut(param1:MouseEvent) : void
      {
         if(!param1 || !param1.target)
         {
            return;
         }
         var _loc2_:* = param1.target;
         dispatchEvent(new ListEvent(ListEvent.ITEM_ROLL_OUT,true,false,param1.target.index,-1,-1,param1.target as IListItemRenderer,param1.target.data));
      }
      
      protected function newItemOver(param1:MouseEvent) : void
      {
         if(!param1 || !param1.target)
         {
            return;
         }
         var _loc2_:* = param1.target;
         dispatchEvent(new ListEvent(ListEvent.ITEM_ROLL_OVER,true,false,param1.target.index,-1,-1,param1.target as IListItemRenderer,param1.target.data));
      }
   }
}

