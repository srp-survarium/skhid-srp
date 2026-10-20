package
{
   import adobe.utils.*;
   import flash.accessibility.*;
   import flash.desktop.*;
   import flash.display.*;
   import flash.errors.*;
   import flash.events.*;
   import flash.external.*;
   import flash.filters.*;
   import flash.geom.*;
   import flash.globalization.*;
   import flash.media.*;
   import flash.net.*;
   import flash.net.drm.*;
   import flash.printing.*;
   import flash.profiler.*;
   import flash.sampler.*;
   import flash.sensors.*;
   import flash.system.*;
   import flash.text.*;
   import flash.text.engine.*;
   import flash.text.ime.*;
   import flash.ui.*;
   import flash.utils.*;
   import flash.xml.*;
   import survarium.controls.ContextList;
   
   public dynamic class SrvContextList extends ContextList
   {
      
      public function SrvContextList()
      {
         super();
         this.__setProp_list_contextList_();
      }
      
      internal function __setProp_list_contextList_() : *
      {
         try
         {
            list["componentInspectorSetting"] = true;
         }
         catch(e:Error)
         {
         }
         list.autoSize = "left";
         list.buttonWidth = 155;
         list.direction = "vertical";
         list.enabled = true;
         list.enableInitCallback = false;
         list.focusable = true;
         list.itemRendererName = "SvButtonHint";
         list.spacing = 0;
         list.visible = true;
         try
         {
            list["componentInspectorSetting"] = false;
         }
         catch(e:Error)
         {
         }
      }
   }
}

