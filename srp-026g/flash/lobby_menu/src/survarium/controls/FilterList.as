package survarium.controls
{
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.events.MouseEvent;
   import scaleform.clik.constants.DirectionMode;
   import scaleform.clik.constants.InputValue;
   import scaleform.clik.constants.NavigationCode;
   import scaleform.clik.constants.WrappingMode;
   import scaleform.clik.controls.Button;
   import scaleform.clik.controls.TileList;
   import scaleform.clik.data.DataProvider;
   import scaleform.clik.events.InputEvent;
   import scaleform.clik.events.ListEvent;
   import scaleform.clik.interfaces.IListItemRenderer;
   import scaleform.clik.ui.InputDetails;
   
   public class FilterList extends TileList
   {
      
      public static const NEW_DATA_PROVIDER:String = "new_data_provider";
      
      public var _body:MovieClip;
      
      protected var _fncCallBack:Function;
      
      protected var _maxCols:int = 5;
      
      protected var _selected:Array = new Array();
      
      protected var _providers:Array = new Array();
      
      protected var _focusIndex:int = 1;
      
      protected var _partDataMode:Boolean = false;
      
      public function FilterList()
      {
         super();
         addEventListener(MouseEvent.MOUSE_DOWN,this.inFocusM6);
      }
      
      public function registerCallBack(param1:Function) : void
      {
         _fncCallBack = param1;
      }
      
      public function registerDataProvider(param1:uint, param2:DataProvider) : void
      {
         this._providers[param1] = param2;
      }
      
      public function getDataProviderCount() : uint
      {
         return this._providers.length;
      }
      
      public function getDataProvider(param1:uint) : DataProvider
      {
         return this._providers[param1];
      }
      
      public function clear_providers() : void
      {
         this._providers.length = 0;
      }
      
      override protected function configUI() : void
      {
         super.configUI();
      }
      
      protected function inFocusM6(param1:MouseEvent) : void
      {
         focusIndex = -1;
      }
      
      override protected function changeFocus() : void
      {
         if(!_focused)
         {
            focusIndex = -1;
         }
         else if(selectedIndex >= dataProvider.length || selectedIndex == -1)
         {
            focusIndex = 0;
         }
         else
         {
            focusIndex = selectedIndex;
         }
      }
      
      public function setDataProvider(param1:uint) : void
      {
         var _loc2_:uint;
         if(dataProvider == this._providers[param1])
         {
            return;
         }
         _partDataMode = false;
         dataProvider = this._providers[param1];
         if(dataProvider)
         {
            _loc2_ = 0;
            while(_loc2_ < dataProvider.length)
            {
               dataProvider[_loc2_].enabled = true;
               _loc2_++;
            }
         }
         dispatchEvent(new Event(NEW_DATA_PROVIDER));
      }
      
      public function set focusIndex(param1:int) : void
      {
         if(param1 == this._focusIndex)
         {
            return;
         }
         if(!_renderers || _renderers.length == 0)
         {
            return;
         }
         var _loc2_:int = this._focusIndex;
         var _loc3_:Button = _renderers[_loc2_] as Button;
         if(_loc3_)
         {
            _loc3_.displayFocus = false;
         }
         _focusIndex = param1;
         _loc3_ = _renderers[this._focusIndex] as Button;
         if(_loc3_)
         {
            _loc3_.displayFocus = true;
         }
      }
      
      override public function handleInput(param1:InputEvent) : void
      {
         if(param1.handled)
         {
            return;
         }
         var _loc2_:IListItemRenderer = getRendererAt(this._focusIndex,_scrollPosition);
         if(_loc2_ != null)
         {
            _loc2_.handleInput(param1);
            if(param1.handled)
            {
               return;
            }
         }
         var _loc3_:InputDetails = param1.details;
         var _loc4_:Boolean = _loc3_.value == InputValue.KEY_DOWN || _loc3_.value == InputValue.KEY_HOLD;
         var _loc5_:int = -1;
         var _loc6_:String = _loc3_.navEquivalent;
         if(_direction == DirectionMode.HORIZONTAL)
         {
            switch(_loc6_)
            {
               case NavigationCode.RIGHT:
                  _loc5_ = this._focusIndex + _totalRows;
                  break;
               case NavigationCode.LEFT:
                  _loc5_ = this._focusIndex - _totalRows;
                  break;
               case NavigationCode.UP:
                  _loc5_ = this._focusIndex - 1;
                  break;
               case NavigationCode.DOWN:
                  _loc5_ = this._focusIndex + 1;
               break;
            }
         }
         else
         {
            switch(_loc6_)
            {
               case NavigationCode.DOWN:
                  _loc5_ = this._focusIndex + _totalColumns;
                  break;
               case NavigationCode.UP:
                  _loc5_ = this._focusIndex - _totalColumns;
                  break;
               case NavigationCode.LEFT:
                  _loc5_ = this._focusIndex - 1;
                  break;
               case NavigationCode.RIGHT:
                  _loc5_ = this._focusIndex + 1;
               break;
            }
         }
         if(_loc5_ == -1)
         {
            switch(_loc6_)
            {
               case NavigationCode.HOME:
                  _loc5_ = 0;
                  break;
               case NavigationCode.END:
                  _loc5_ = _dataProvider.length - 1;
                  break;
               case NavigationCode.PAGE_DOWN:
                  _loc5_ = Math.min(_dataProvider.length - 1,this._focusIndex + _totalColumns * _totalRows);
                  break;
               case NavigationCode.PAGE_UP:
                  _loc5_ = Math.max(0,this._focusIndex - _totalColumns * _totalRows);
               break;
            }
         }
         if(_loc5_ != -1)
         {
            if(!_loc4_)
            {
               param1.handled = true;
               return;
            }
            if(_loc5_ >= 0 && _loc5_ < dataProvider.length)
            {
               focusIndex = Math.max(0,Math.min(_dataProvider.length - 1,_loc5_));
               param1.handled = true;
            }
            else if(wrapping == WrappingMode.STICK)
            {
               _loc5_ = Math.max(0,Math.min(_dataProvider.length - 1,_loc5_));
               if(this._focusIndex != _loc5_)
               {
                  focusIndex = _loc5_;
               }
               param1.handled = true;
            }
            else if(wrapping == WrappingMode.WRAP)
            {
               focusIndex = _loc5_ < 0 ? (_dataProvider.length - 1) : (this._focusIndex < _dataProvider.length - 1 ? (_dataProvider.length - 1) : 0);
               param1.handled = true;
            }
         }
      }
      
      public function getSelectedPositions() : Array
      {
         var _loc1_:Array = [];
         var _loc2_:uint;
         if(dataProvider)
         {
            _loc2_ = 0;
            while(_loc2_ < dataProvider.length)
            {
               if(dataProvider[_loc2_].selected == true)
               {
                  _loc1_.push(_loc2_);
               }
               _loc2_++;
            }
         }
         return _loc1_;
      }
      
      public function getSelectedIndexes() : Array
      {
         var _loc1_:uint;
         var _loc2_:Object;
         var _loc3_:uint;
         this._selected.length = 0;
         if(dataProvider)
         {
            _loc1_ = dataProvider.length;
            _loc3_ = 0;
            while(_loc3_ < _loc1_)
            {
               _loc2_ = dataProvider[_loc3_];
               if(_loc2_.selected == true)
               {
                  this._selected.push(_loc2_.sucat);
               }
               _loc3_++;
            }
            trace("getSelectedIndexes",this._partDataMode,this._selected.length);
            if(this._partDataMode && this._selected.length == 0)
            {
               _loc3_ = 0;
               while(_loc3_ < _loc1_)
               {
                  _loc2_ = dataProvider[_loc3_];
                  trace("iterateData::",_loc2_.enabled);
                  if(_loc2_.enabled != false)
                  {
                     this._selected.push(_loc2_.sucat);
                  }
                  _loc3_++;
               }
            }
         }
         return this._selected;
      }
      
      override public function set selectedIndex(param1:int) : void
      {
         _newSelectedIndex = param1;
         invalidateSelectedIndex();
         focusIndex = param1;
         dispatchEvent(new ListEvent(ListEvent.INDEX_CHANGE,true,false,_selectedIndex,-1,-1,getRendererAt(_selectedIndex),dataProvider[_selectedIndex]));
      }
      
      override protected function refreshData() : void
      {
         var _loc1_:Number = _direction == DirectionMode.HORIZONTAL ? _totalRows : _totalColumns;
         var _loc2_:Number = Math.ceil(_dataProvider.length / _loc1_);
         var _loc3_:Number = _loc2_ - (_direction == DirectionMode.HORIZONTAL ? _totalColumns : _totalRows);
         _scrollPosition = Math.max(0,Math.min(_loc3_,_scrollPosition));
         var _loc4_:Number = _scrollPosition * _loc1_;
         var _loc5_:Number = _loc4_ + _totalColumns * _totalRows - 1;
         _dataProvider.requestItemRange(_loc4_,_loc5_,populateData);
      }
      
      public function focusPart(param1:Array, param2:uint = 0) : void
      {
         trace("focusPart",param1,"id:",param2,_selectedIndex);
         _partDataMode = true;
         var _loc3_:uint = 0;
         while(_loc3_ < this._providers[param2].length)
         {
            this._providers[param2][_loc3_].selected = false;
            this._providers[param2][_loc3_].enabled = false;
            _loc3_++;
         }
         _loc3_ = 0;
         while(_loc3_ < param1.length)
         {
            this._providers[param2][param1[_loc3_]].selected = true;
            this._providers[param2][param1[_loc3_]].enabled = true;
            _loc3_++;
         }
         this.refreshData();
         dispatchEvent(new ListEvent(ListEvent.INDEX_CHANGE,true,false,_selectedIndex,-1,-1,getRendererAt(_selectedIndex),dataProvider[_selectedIndex]));
      }
      
      public function selectPart(param1:Array, param2:uint = 0, param3:Boolean = false) : void
      {
         trace("selectPart",param1,"id:",param2,_selectedIndex);
         var _loc4_:uint = 0;
         while(_loc4_ < this._providers[param2].length)
         {
            this._providers[param2][_loc4_].selected = false;
            _loc4_++;
         }
         _loc4_ = 0;
         while(_loc4_ < param1.length)
         {
            this._providers[param2][param1[_loc4_]].selected = true;
            _loc4_++;
         }
         this.refreshData();
         dispatchEvent(new ListEvent(ListEvent.INDEX_CHANGE,true,false,_selectedIndex,-1,-1,getRendererAt(_selectedIndex),dataProvider[_selectedIndex]));
      }
      
      public function deselectAll() : void
      {
         trace("deselectAll");
         var _loc1_:uint = 0;
         var _loc2_:IListItemRenderer;
         while(_loc1_ < _renderers.length)
         {
            _loc2_ = getRendererAt(_loc1_,scrollPosition);
            _loc2_.selected = false;
            _loc1_++;
         }
      }
      
      public function selectAll() : void
      {
         trace("selectAll");
         var _loc1_:uint = 0;
         var _loc2_:IListItemRenderer;
         while(_loc1_ < _renderers.length)
         {
            _loc2_ = getRendererAt(_loc1_,scrollPosition);
            _loc2_.selected = true;
            _loc2_.validateNow();
            _loc1_++;
         }
      }
      
      override protected function updateSelectedIndex() : void
      {
         if(_totalRenderers == 0)
         {
            return;
         }
         var _loc1_:IListItemRenderer = getRendererAt(_selectedIndex,scrollPosition);
         if(_loc1_ != null)
         {
            _loc1_.validateNow();
         }
         _selectedIndex = _newSelectedIndex;
         invalidateSelectedIndex();
         if(_selectedIndex < 0 || _selectedIndex >= _dataProvider.length)
         {
            return;
         }
         _loc1_ = getRendererAt(_selectedIndex,_scrollPosition);
         if(_loc1_ != null)
         {
            _loc1_.selected = !_loc1_.selected;
            _loc1_.validateNow();
         }
         else
         {
            scrollToIndex(_selectedIndex);
            _loc1_ = getRendererAt(_selectedIndex,scrollPosition);
            _loc1_.selected = !_loc1_.selected;
            _loc1_.validateNow();
         }
         dispatchEvent(new ListEvent(ListEvent.INDEX_CHANGE,true,false,_selectedIndex,-1,-1,getRendererAt(_selectedIndex),dataProvider[_selectedIndex]));
      }
   }
}

