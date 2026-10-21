package profiles_fla
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
   
   public dynamic class MainTimeline extends MovieClip
   {
      
      public var profileTab2:SrvPlayerProfileTab;
      
      public var btnChangeName:BtnRenameProfile;
      
      public var profileTab:MovieClip;
      
      public function MainTimeline()
      {
         super();
         addFrameScript(0,this.frame1);
      }
      
      public function test() : *
      {
         this.profileTab.data = {
            label:"IX|set_1",
            state:0
         };
         this.profileTab.invalidateData();
      }
      
      internal function frame1() : *
      {
         this.profileTab.data = {
            label:"IX|set_1",
            state:2
         };
         this.profileTab.invalidateData();
         setTimeout(this.test,2000);
      }
   }
}

